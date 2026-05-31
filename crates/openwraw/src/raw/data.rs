// Reader for _FUNCnnn.DAT - the binary spectrum data files.
// Spectra are stored contiguously, referenced by offsets from the
// paired .IDX file. Multiple compression schemes are known to exist
// across instrument generations; scheme detection is done per-spectrum.

use crate::bytes::read_u16_le;
use crate::raw::header::FunctionCal;

/// Parameters required by all three DAT decoders.
///
/// Construct one `DecodeParams` per function per run by combining the
/// outputs of `Header` (calibration polynomial), `ExternInf` (A_us),
/// and `FunctionInfo` (mass range, scan time).
pub struct DecodeParams {
    /// TOF constant A (µs / sqrt(Da)).  Computed by `ExternInf::a_us()`.
    pub a_us: f64,
    /// Per-function T1 calibration polynomial from `_HEADER.TXT`.
    pub cal: FunctionCal,
    /// Acquisition m/z lower bound (Da), from `_FUNCTNS.INF` +0x0A0.
    pub mz_low: f64,
    /// Acquisition m/z upper bound (Da), from `_FUNCTNS.INF` +0x120.
    pub mz_high: f64,
    /// Scan duration (ms), from `_FUNCTNS.INF` +0x020 × 1000.
    /// Used only by Encoding B to convert dt_bin to drift time.
    pub scan_time_ms: f64,
}

/// Decoded spectrum from a non-IMS scan (Encoding A or C).
#[derive(Debug, Default, Clone)]
pub struct Spectrum {
    pub mz: Vec<f64>,
    pub intensity: Vec<f32>,
}

/// Decoded spectrum from an IMS scan (Encoding B).
#[derive(Debug, Default, Clone)]
pub struct ImsSpectrum {
    pub mz: Vec<f64>,
    pub drift_time_ms: Vec<f64>,
    pub intensity: Vec<f32>,
}

// ── Encoding A ────────────────────────────────────────────────────────────────

/// Byte marker for an Encoding A sentinel record.
const ENC_A_SENTINEL: u8 = 0x70;

/// Decode one scan slice from an Encoding A `_FUNCnnn.DAT` file.
///
/// `scan_bytes` must be the exact bytes of one scan as given by the paired
/// `_FUNCnnn.IDX` Variant A record (offset, n_records × 6).
///
/// The first record of every scan is a sentinel (block_type == 0x70); its
/// `tof_bin` field encodes the scale factor for the entire scan.
pub fn decode_encoding_a(scan_bytes: &[u8], params: &DecodeParams) -> crate::Result<Spectrum> {
    if scan_bytes.is_empty() {
        return Ok(Spectrum::default());
    }
    if scan_bytes.len() % 6 != 0 {
        return Err(crate::Error::Parse(format!(
            "Encoding A: scan size {} is not a multiple of 6",
            scan_bytes.len()
        )));
    }

    let n = scan_bytes.len() / 6;

    // First record must be the sentinel.
    if scan_bytes[2] != ENC_A_SENTINEL {
        return Err(crate::Error::Parse(format!(
            "Encoding A: first record block_type {:#04x} is not sentinel ({:#04x})",
            scan_bytes[2], ENC_A_SENTINEL
        )));
    }
    let sentinel_tof_bin = read_u16_le(scan_bytes, 4)? as f64;
    if sentinel_tof_bin == 0.0 {
        return Err(crate::Error::Parse(
            "Encoding A: sentinel_tof_bin is zero".to_owned(),
        ));
    }

    // t_bin_us: µs per TOF bin for this scan.
    // A_us * sqrt(mz_high) gives the expected flight time at mz_high; that
    // flight time corresponds to sentinel_tof_bin bins.
    let t_bin_us = params.a_us * params.mz_high.sqrt() / sentinel_tof_bin;

    let mut out = Spectrum {
        mz: Vec::with_capacity(n.saturating_sub(1)),
        intensity: Vec::with_capacity(n.saturating_sub(1)),
    };

    for i in 1..n {
        let rec = &scan_bytes[i * 6..(i + 1) * 6];
        let block_type = rec[2];
        let raw_intensity = rec[3];
        let tof_bin = read_u16_le(rec, 4)? as f64;

        if block_type == ENC_A_SENTINEL || raw_intensity == 0 {
            continue;
        }

        let t_raw = tof_bin * t_bin_us;
        let t_cal = params.cal.apply(t_raw);
        out.mz.push((t_cal / params.a_us).powi(2));
        out.intensity.push(raw_intensity as f32);
    }

    Ok(out)
}

// ── Encoding B ────────────────────────────────────────────────────────────────

/// Decode one scan slice from an Encoding B `_FUNCnnn.DAT` file (IMS mode).
///
/// `scan_bytes` must be the exact bytes of one scan as given by the paired
/// `_FUNCnnn.IDX` Variant B record (offset at +0x16; length from next offset).
///
/// The first and last record's `tof_bin` fields anchor the TOF bin→µs scale.
/// Records with `count == 0` (sentinels) are skipped in the output.
pub fn decode_encoding_b(scan_bytes: &[u8], params: &DecodeParams) -> crate::Result<ImsSpectrum> {
    if scan_bytes.is_empty() {
        return Ok(ImsSpectrum::default());
    }
    if scan_bytes.len() % 8 != 0 {
        return Err(crate::Error::Parse(format!(
            "Encoding B: scan size {} is not a multiple of 8",
            scan_bytes.len()
        )));
    }

    let n = scan_bytes.len() / 8;
    if n < 2 {
        // Cannot derive the bin→time scale from a single record.
        return Ok(ImsSpectrum::default());
    }

    let tof_bin_low = read_u16_le(scan_bytes, 6)? as f64;
    let last = &scan_bytes[(n - 1) * 8..n * 8];
    let tof_bin_high = read_u16_le(last, 6)? as f64;

    if tof_bin_high <= tof_bin_low {
        // Empty or degenerate scan; return empty without error.
        return Ok(ImsSpectrum::default());
    }

    let t_low = params.a_us * params.mz_low.sqrt();
    let t_high = params.a_us * params.mz_high.sqrt();
    let t_bin = (t_high - t_low) / (tof_bin_high - tof_bin_low);

    let mut out = ImsSpectrum {
        mz: Vec::with_capacity(n),
        drift_time_ms: Vec::with_capacity(n),
        intensity: Vec::with_capacity(n),
    };

    for i in 0..n {
        let rec = &scan_bytes[i * 8..(i + 1) * 8];
        let count = read_u16_le(rec, 2)?;
        let dt_bin = read_u16_le(rec, 4)? as f64;
        let tof_bin = read_u16_le(rec, 6)? as f64;

        if count == 0 {
            continue;
        }

        let t_raw = t_low + (tof_bin - tof_bin_low) * t_bin;
        let t_cal = params.cal.apply(t_raw);
        out.mz.push((t_cal / params.a_us).powi(2));
        out.drift_time_ms
            .push(dt_bin * params.scan_time_ms / 65536.0);
        out.intensity.push(count as f32);
    }

    Ok(out)
}

// ── Encoding C ────────────────────────────────────────────────────────────────

/// Decode one scan slice from an Encoding C `_FUNCnnn.DAT` file (non-IMS QTof).
///
/// `scan_bytes` must be the exact bytes of one scan as given by the paired
/// `_FUNCnnn.IDX` Variant B record (offset at +0x16; length from next offset).
///
/// The first record's `tof_bin` = mz_low_bin; the last record's `tof_bin` =
/// mz_high_bin.  Zero-intensity records (sentinels) are skipped in the output.
/// The `sub_bin` field (bytes 4-5) provides fractional TOF bin position.
pub fn decode_encoding_c(scan_bytes: &[u8], params: &DecodeParams) -> crate::Result<Spectrum> {
    if scan_bytes.is_empty() {
        return Ok(Spectrum::default());
    }
    if scan_bytes.len() % 8 != 0 {
        return Err(crate::Error::Parse(format!(
            "Encoding C: scan size {} is not a multiple of 8",
            scan_bytes.len()
        )));
    }

    let n = scan_bytes.len() / 8;
    if n < 2 {
        return Ok(Spectrum::default());
    }

    let tof_bin_low = read_u16_le(scan_bytes, 6)? as f64;
    let last = &scan_bytes[(n - 1) * 8..n * 8];
    let tof_bin_high = read_u16_le(last, 6)? as f64;

    if tof_bin_high <= tof_bin_low {
        return Ok(Spectrum::default());
    }

    let t_low = params.a_us * params.mz_low.sqrt();
    let t_high = params.a_us * params.mz_high.sqrt();
    let t_bin = (t_high - t_low) / (tof_bin_high - tof_bin_low);

    let mut out = Spectrum {
        mz: Vec::with_capacity(n.saturating_sub(2)),
        intensity: Vec::with_capacity(n.saturating_sub(2)),
    };

    for i in 0..n {
        let rec = &scan_bytes[i * 8..(i + 1) * 8];
        // bytes[0:2] always 0x0000 for Encoding C (no drift axis)
        let intensity = read_u16_le(rec, 2)?;
        let sub_bin = read_u16_le(rec, 4)? as f64;
        let tof_bin = read_u16_le(rec, 6)? as f64;

        if intensity == 0 {
            continue;
        }

        let frac_bin = (tof_bin - tof_bin_low) + sub_bin / 65536.0;
        let t_raw = t_low + frac_bin * t_bin;
        let t_cal = params.cal.apply(t_raw);
        out.mz.push((t_cal / params.a_us).powi(2));
        out.intensity.push(intensity as f32);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::header::{CalType, FunctionCal};

    // Identity calibration: t_cal = t_raw (c0=0, c1=1)
    fn identity_cal() -> FunctionCal {
        FunctionCal {
            coeffs: vec![0.0, 1.0],
            cal_type: CalType::T1,
        }
    }

    // Params for easy mental arithmetic:
    //   a_us=1.0 µs/sqrt(Da), mz_low=4.0, mz_high=100.0
    //   t_low=2.0 µs, t_high=10.0 µs
    //   scan_time_ms=1000.0
    fn test_params() -> DecodeParams {
        DecodeParams {
            a_us: 1.0,
            cal: identity_cal(),
            mz_low: 4.0,
            mz_high: 100.0,
            scan_time_ms: 1000.0,
        }
    }

    // ── Encoding A helpers ──────────────────────────────────────────────────

    fn enc_a_sentinel(sentinel_tof_bin: u16) -> [u8; 6] {
        let mut r = [0u8; 6];
        r[2] = ENC_A_SENTINEL;
        r[4..6].copy_from_slice(&sentinel_tof_bin.to_le_bytes());
        r
    }

    fn enc_a_data(block_type: u8, intensity: u8, tof_bin: u16) -> [u8; 6] {
        let mut r = [0u8; 6];
        r[2] = block_type;
        r[3] = intensity;
        r[4..6].copy_from_slice(&tof_bin.to_le_bytes());
        r
    }

    // ── Encoding B/C helpers ────────────────────────────────────────────────

    fn enc_b_record(count: u16, dt_bin: u16, tof_bin: u16) -> [u8; 8] {
        let mut r = [0u8; 8];
        r[2..4].copy_from_slice(&count.to_le_bytes());
        r[4..6].copy_from_slice(&dt_bin.to_le_bytes());
        r[6..8].copy_from_slice(&tof_bin.to_le_bytes());
        r
    }

    fn enc_c_record(intensity: u16, sub_bin: u16, tof_bin: u16) -> [u8; 8] {
        let mut r = [0u8; 8];
        r[2..4].copy_from_slice(&intensity.to_le_bytes());
        r[4..6].copy_from_slice(&sub_bin.to_le_bytes());
        r[6..8].copy_from_slice(&tof_bin.to_le_bytes());
        r
    }

    fn bytes_of<const N: usize>(recs: &[[u8; N]]) -> Vec<u8> {
        recs.iter().flat_map(|r| r.iter().copied()).collect()
    }

    // ── Encoding A tests ────────────────────────────────────────────────────

    // With a_us=1.0, mz_high=100.0, sentinel_tof_bin=10000:
    //   t_bin_us = 1.0 * 10.0 / 10000 = 0.001 µs/bin
    //   tof_bin=6000 → t_raw=6.0 µs → mz=(6.0/1.0)^2=36.0 Da
    #[test]
    fn enc_a_decodes_peak_mz() {
        let scan = bytes_of(&[enc_a_sentinel(10000), enc_a_data(0x80, 42, 6000)]);
        let spec = decode_encoding_a(&scan, &test_params()).unwrap();
        assert_eq!(spec.mz.len(), 1);
        assert!((spec.mz[0] - 36.0).abs() < 1e-8, "mz={}", spec.mz[0]);
        assert_eq!(spec.intensity[0], 42.0);
    }

    #[test]
    fn enc_a_skips_zero_intensity() {
        let scan = bytes_of(&[
            enc_a_sentinel(10000),
            enc_a_data(0x80, 0, 6000), // intensity=0 → skip
            enc_a_data(0x80, 10, 7000),
        ]);
        let spec = decode_encoding_a(&scan, &test_params()).unwrap();
        assert_eq!(spec.mz.len(), 1);
    }

    #[test]
    fn enc_a_sentinel_only_is_empty() {
        let scan = bytes_of(&[enc_a_sentinel(10000)]);
        let spec = decode_encoding_a(&scan, &test_params()).unwrap();
        assert!(spec.mz.is_empty());
    }

    #[test]
    fn enc_a_empty_bytes_is_empty() {
        let spec = decode_encoding_a(&[], &test_params()).unwrap();
        assert!(spec.mz.is_empty());
    }

    #[test]
    fn enc_a_bad_first_record_is_error() {
        // First record has block_type=0x80 (not sentinel)
        let scan = bytes_of(&[enc_a_data(0x80, 10, 5000)]);
        assert!(decode_encoding_a(&scan, &test_params()).is_err());
    }

    #[test]
    fn enc_a_bad_size_is_error() {
        let data = vec![0u8; 7]; // not multiple of 6
        assert!(decode_encoding_a(&data, &test_params()).is_err());
    }

    #[test]
    fn enc_a_mz_within_declared_range() {
        let scan = bytes_of(&[
            enc_a_sentinel(10000),
            enc_a_data(0x80, 1, 2001), // tof_bin just above t_low
            enc_a_data(0x80, 1, 9999), // tof_bin just below sentinel
        ]);
        let p = test_params();
        let spec = decode_encoding_a(&scan, &p).unwrap();
        for &m in &spec.mz {
            assert!(m >= p.mz_low, "mz={m} < mz_low={}", p.mz_low);
            assert!(m <= p.mz_high * 1.01, "mz={m} > mz_high={}", p.mz_high);
        }
    }

    // ── Encoding B tests ────────────────────────────────────────────────────

    // With a_us=1.0, mz_low=4.0, mz_high=100.0:
    //   t_low=2.0, t_high=10.0, tof_bin_low=2000, tof_bin_high=10000
    //   t_bin = 8.0/8000 = 0.001 µs/bin
    //   tof_bin=6000 → t_raw=2.0+(6000-2000)*0.001=6.0 µs → mz=36.0 Da
    //   dt_bin=3000, scan_time_ms=1000 → drift=3000*1000/65536≈45.8 ms
    #[test]
    fn enc_b_decodes_peak_mz_and_drift() {
        let scan = bytes_of(&[
            enc_b_record(0, 0, 2000),    // first (sentinel, count=0, tof_bin_low)
            enc_b_record(5, 3000, 6000), // data
            enc_b_record(0, 0, 10000),   // last (sentinel, count=0, tof_bin_high)
        ]);
        let spec = decode_encoding_b(&scan, &test_params()).unwrap();
        assert_eq!(spec.mz.len(), 1);
        assert!((spec.mz[0] - 36.0).abs() < 1e-8, "mz={}", spec.mz[0]);
        assert_eq!(spec.intensity[0], 5.0);
        let expected_drift = 3000.0 * 1000.0 / 65536.0;
        assert!((spec.drift_time_ms[0] - expected_drift).abs() < 1e-6);
    }

    #[test]
    fn enc_b_skips_zero_count_sentinel() {
        let scan = bytes_of(&[
            enc_b_record(0, 0, 2000),   // sentinel low
            enc_b_record(3, 100, 5000), // data
            enc_b_record(0, 0, 10000),  // sentinel high
        ]);
        let spec = decode_encoding_b(&scan, &test_params()).unwrap();
        assert_eq!(spec.mz.len(), 1);
    }

    #[test]
    fn enc_b_empty_bytes_is_empty() {
        let spec = decode_encoding_b(&[], &test_params()).unwrap();
        assert!(spec.mz.is_empty());
    }

    #[test]
    fn enc_b_all_zero_count_is_empty_output() {
        // scan where every record has count=0 (blank scan)
        let scan = bytes_of(&[
            enc_b_record(0, 0, 2000),
            enc_b_record(0, 100, 6000),
            enc_b_record(0, 0, 10000),
        ]);
        let spec = decode_encoding_b(&scan, &test_params()).unwrap();
        assert!(spec.mz.is_empty());
    }

    #[test]
    fn enc_b_bad_size_is_error() {
        let data = vec![0u8; 9]; // not multiple of 8
        assert!(decode_encoding_b(&data, &test_params()).is_err());
    }

    // ── Encoding C tests ────────────────────────────────────────────────────

    // Same calibration as Encoding B.
    // sub_bin=0 → frac_bin = tof_bin - tof_bin_low, same formula as B.
    // sub_bin=32768 → adds 0.5 to frac_bin.
    #[test]
    fn enc_c_decodes_peak_mz_no_subbin() {
        let scan = bytes_of(&[
            enc_c_record(0, 0, 2000),  // sentinel low
            enc_c_record(7, 0, 6000),  // data, sub_bin=0
            enc_c_record(0, 0, 10000), // sentinel high
        ]);
        let spec = decode_encoding_c(&scan, &test_params()).unwrap();
        assert_eq!(spec.mz.len(), 1);
        // frac_bin = 4000 + 0 = 4000 → t_raw=2.0+4000*0.001=6.0 → mz=36.0
        assert!((spec.mz[0] - 36.0).abs() < 1e-8, "mz={}", spec.mz[0]);
        assert_eq!(spec.intensity[0], 7.0);
    }

    #[test]
    fn enc_c_subbin_gives_finer_mz_than_no_subbin() {
        let scan_no_sub = bytes_of(&[
            enc_c_record(0, 0, 2000),
            enc_c_record(1, 0, 6000), // sub_bin=0
            enc_c_record(0, 0, 10000),
        ]);
        let scan_half_sub = bytes_of(&[
            enc_c_record(0, 0, 2000),
            enc_c_record(1, 32768, 6000), // sub_bin=32768 → +0.5 bin
            enc_c_record(0, 0, 10000),
        ]);
        let p = test_params();
        let spec_no = decode_encoding_c(&scan_no_sub, &p).unwrap();
        let spec_sub = decode_encoding_c(&scan_half_sub, &p).unwrap();
        // sub_bin=32768 shifts frac_bin by +0.5, so mz should be slightly higher.
        assert!(spec_sub.mz[0] > spec_no.mz[0]);
        // Difference should be small (~0.01 Da at mz=36)
        assert!((spec_sub.mz[0] - spec_no.mz[0]) < 0.1);
    }

    #[test]
    fn enc_c_skips_zero_intensity_sentinels() {
        let scan = bytes_of(&[
            enc_c_record(0, 0, 2000),  // sentinel
            enc_c_record(5, 0, 5000),  // data
            enc_c_record(0, 0, 10000), // sentinel
        ]);
        let spec = decode_encoding_c(&scan, &test_params()).unwrap();
        assert_eq!(spec.mz.len(), 1);
    }

    #[test]
    fn enc_c_empty_bytes_is_empty() {
        let spec = decode_encoding_c(&[], &test_params()).unwrap();
        assert!(spec.mz.is_empty());
    }

    #[test]
    fn enc_c_bad_size_is_error() {
        let data = vec![0u8; 11]; // not multiple of 8
        assert!(decode_encoding_c(&data, &test_params()).is_err());
    }

    // ── Corpus integration tests ────────────────────────────────────────────
    // These tests read from the local corpus and are skipped when it is absent.

    #[test]
    fn corpus_encoding_a_pxd058812() {
        use crate::raw::{extern_inf::ExternInf, functions_inf::FunctionTable, index::ScanIndex};
        use std::path::Path;

        let raw = Path::new("/workspaces/OpenWRaw/corpus/PXD058812/molecular_mass_P15_01.raw");
        if !raw.exists() {
            return;
        }

        let header = crate::raw::header::Header::from_path(&raw.join("_HEADER.TXT")).unwrap();
        let ext = ExternInf::from_path(&raw.join("_extern.inf")).unwrap();
        let funcs = FunctionTable::from_path(&raw.join("_FUNCTNS.INF")).unwrap();
        let f = &funcs.functions[0];

        let params = DecodeParams {
            a_us: ext.a_us(),
            cal: header.cal_functions[&1].clone(),
            mz_low: f.mz_low as f64,
            mz_high: f.mz_high as f64,
            scan_time_ms: f.scan_time_s as f64 * 1000.0,
        };

        let idx_bytes = std::fs::read(raw.join("_FUNC001.IDX")).unwrap();
        let dat_bytes = std::fs::read(raw.join("_FUNC001.DAT")).unwrap();
        let ScanIndex::A(idx) = ScanIndex::from_bytes(&idx_bytes).unwrap() else {
            panic!("expected Variant A")
        };

        // Scan 3 is the first non-blank scan (scans 0-2 are blank/2-record sentinels).
        let scan3 = &idx[3];
        let scan_bytes = &dat_bytes
            [scan3.dat_offset as usize..(scan3.dat_offset + scan3.n_records as u32 * 6) as usize];
        let spec = decode_encoding_a(scan_bytes, &params).unwrap();

        assert!(!spec.mz.is_empty(), "scan 3 should have peaks");
        // NOTE: tof_bin can exceed sentinel_tof_bin (max observed 65236 > sentinel 51199)
        // so decoded mz can exceed the declared mz_high from FUNCTNS.INF.
        // Only check the lower bound and physical plausibility.
        for &m in &spec.mz {
            assert!(m > 0.0 && m.is_finite(), "mz={m} is not positive/finite");
            assert!(m >= params.mz_low * 0.9, "mz={m} unreasonably below mz_low");
            assert!(m < 50000.0, "mz={m} unreasonably large");
        }
        for &i in &spec.intensity {
            assert!(i > 0.0, "zero intensity should have been filtered");
        }
    }

    #[test]
    fn corpus_encoding_b_pxd068881() {
        use crate::raw::{extern_inf::ExternInf, functions_inf::FunctionTable, index::ScanIndex};
        use std::path::Path;

        let raw = Path::new("/workspaces/OpenWRaw/corpus/PXD068881/20220517_CtpA_1076_2h_1.raw");
        if !raw.exists() {
            return;
        }

        let header = crate::raw::header::Header::from_path(&raw.join("_HEADER.TXT")).unwrap();
        let ext = ExternInf::from_path(&raw.join("_extern.inf")).unwrap();
        let funcs = FunctionTable::from_path(&raw.join("_FUNCTNS.INF")).unwrap();
        let f = &funcs.functions[0];

        let params = DecodeParams {
            a_us: ext.a_us(),
            cal: header.cal_functions[&1].clone(),
            mz_low: f.mz_low as f64,
            mz_high: f.mz_high as f64,
            scan_time_ms: f.scan_time_s as f64 * 1000.0,
        };

        let idx_bytes = std::fs::read(raw.join("_FUNC001.IDX")).unwrap();
        let dat_bytes = std::fs::read(raw.join("_FUNC001.DAT")).unwrap();
        let ScanIndex::B(idx) = ScanIndex::from_bytes(&idx_bytes).unwrap() else {
            panic!("expected Variant B")
        };

        // Find the first scan with at least one non-zero-count record.
        let mut found_data = false;
        for (i, rec) in idx.iter().enumerate() {
            let start = rec.dat_offset as usize;
            let end = idx
                .get(i + 1)
                .map(|r| r.dat_offset as usize)
                .unwrap_or(dat_bytes.len());
            if end <= start {
                continue;
            }
            let scan_bytes = &dat_bytes[start..end];
            let spec = decode_encoding_b(scan_bytes, &params).unwrap();
            if spec.mz.is_empty() {
                continue;
            }
            found_data = true;
            for &m in &spec.mz {
                assert!(m >= params.mz_low * 0.99, "mz={m} below mz_low");
                assert!(m <= params.mz_high * 1.01, "mz={m} above mz_high");
            }
            for &d in &spec.drift_time_ms {
                assert!(
                    d >= 0.0 && d <= params.scan_time_ms,
                    "drift={d} out of range"
                );
            }
            break;
        }
        assert!(found_data, "no scan with IMS data found in function 1");
    }

    #[test]
    fn corpus_encoding_c_pxd075602() {
        use crate::raw::{extern_inf::ExternInf, functions_inf::FunctionTable, index::ScanIndex};
        use std::path::Path;

        let raw = Path::new("/workspaces/OpenWRaw/corpus/PXD075602/DHPR_11257-1.raw");
        if !raw.exists() {
            return;
        }

        let header = crate::raw::header::Header::from_path(&raw.join("_HEADER.TXT")).unwrap();
        let ext = ExternInf::from_path(&raw.join("_extern.inf")).unwrap();
        let funcs = FunctionTable::from_path(&raw.join("_FUNCTNS.INF")).unwrap();
        let f = &funcs.functions[0];

        let params = DecodeParams {
            a_us: ext.a_us(),
            cal: header.cal_functions[&1].clone(),
            mz_low: f.mz_low as f64,
            mz_high: f.mz_high as f64,
            scan_time_ms: f.scan_time_s as f64 * 1000.0,
        };

        let idx_bytes = std::fs::read(raw.join("_FUNC001.IDX")).unwrap();
        let dat_bytes = std::fs::read(raw.join("_FUNC001.DAT")).unwrap();
        let ScanIndex::B(idx) = ScanIndex::from_bytes(&idx_bytes).unwrap() else {
            panic!("expected Variant B")
        };

        // Scan 575 is mid-gradient (RT≈10 min) and expected to have signal.
        // Enumerate from scan 575 and take the first non-empty one.
        let mut found_data = false;
        for i in 575..idx.len() {
            let start = idx[i].dat_offset as usize;
            let end = idx
                .get(i + 1)
                .map(|r| r.dat_offset as usize)
                .unwrap_or(dat_bytes.len());
            let scan_bytes = &dat_bytes[start..end];
            let spec = decode_encoding_c(scan_bytes, &params).unwrap();
            if spec.mz.is_empty() {
                continue;
            }
            found_data = true;
            for &m in &spec.mz {
                assert!(
                    m >= params.mz_low * 0.99,
                    "mz={m} below mz_low={}",
                    params.mz_low
                );
                assert!(
                    m <= params.mz_high * 1.01,
                    "mz={m} above mz_high={}",
                    params.mz_high
                );
            }
            for &inten in &spec.intensity {
                assert!(inten > 0.0, "zero-intensity record should be filtered");
            }
            break;
        }
        assert!(
            found_data,
            "no non-empty Encoding C scan found near scan 575"
        );
    }
}
