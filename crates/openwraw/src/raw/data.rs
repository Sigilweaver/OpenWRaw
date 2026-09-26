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

// -- Encoding A --

/// Uncalibrated m/z stored in bytes 2-5 of an Encoding A record.
///
/// Byte 2 carries a 4-bit exponent in its high nibble (low nibble zero);
/// bytes 3-5 are a normalized 24-bit mantissa with bit 23 set, so
/// `m/z = mantissa * 2^(exponent - 24)`. The first and last records of a
/// scan decode exactly to the acquisition mass range in `_FUNCTNS.INF`.
fn encoding_a_mz(rec: &[u8]) -> crate::Result<f64> {
    let mantissa = u32::from_le_bytes([rec[3], rec[4], rec[5], 0]);
    if rec[2] & 0x0f != 0 || mantissa & 0x80_0000 == 0 {
        return Err(crate::Error::Parse(format!(
            "Encoding A: unsupported m/z word {:02x} {:02x} {:02x} {:02x}",
            rec[2], rec[3], rec[4], rec[5]
        )));
    }
    Ok(f64::from(mantissa) * 2f64.powi(i32::from(rec[2] >> 4) - 24))
}

/// Decode one scan slice from an Encoding A `_FUNCnnn.DAT` file.
///
/// `scan_bytes` must be the exact bytes of one scan as given by the paired
/// `_FUNCnnn.IDX` Variant A record. Each 6-byte record is a u16 LE ion count
/// followed by a floating-point m/z word (see [`encoding_a_mz`]). Zero-count
/// records, including those marking the ends of the mass range, are skipped.
/// The `_HEADER.TXT` T1 polynomial applies to sqrt(m/z).
pub fn decode_encoding_a(scan_bytes: &[u8], params: &DecodeParams) -> crate::Result<Spectrum> {
    if scan_bytes.len() % 6 != 0 {
        return Err(crate::Error::Parse(format!(
            "Encoding A: scan size {} is not a multiple of 6",
            scan_bytes.len()
        )));
    }
    let n = scan_bytes.len() / 6;
    let mut out = Spectrum {
        mz: Vec::with_capacity(n),
        intensity: Vec::with_capacity(n),
    };
    for rec in scan_bytes.chunks_exact(6) {
        let count = u16::from_le_bytes([rec[0], rec[1]]);
        if count == 0 {
            continue;
        }
        let mz = encoding_a_mz(rec)?;
        out.mz.push(params.cal.apply(mz.sqrt()).powi(2));
        out.intensity.push(f32::from(count));
    }
    Ok(out)
}

// -- Encoding B --

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

// -- Encoding C --

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

// -- Encoding D --

/// Bit 26 of the Encoding D position word is always set: the mantissa
/// carries an explicit leading one below a 5-bit exponent.
const ENC_D_LEADING_ONE: u32 = 1 << 26;

/// Uncalibrated m/z stored in an Encoding D position word (bytes 4-7).
///
/// `m/z = 2^(u >> 27) * (u & 0x07FF_FFFF) / 2^27`. Consecutive records in a
/// profile step by one ADC sample, which matches the flight-time model from
/// `Lteff`, `Veff` and the ADC sample frequency across the mass range.
fn encoding_d_mz(u: u32) -> crate::Result<f64> {
    if u & ENC_D_LEADING_ONE == 0 {
        return Err(crate::Error::Parse(format!(
            "Encoding D: position word {u:#010x} lacks the leading mantissa bit"
        )));
    }
    let exponent = (u >> 27) as i32;
    let mantissa = f64::from(u & 0x07FF_FFFF) / f64::from(1u32 << 27);
    Ok(mantissa * 2f64.powi(exponent))
}

/// Decode one scan slice from an Encoding D `_FUNCnnn.DAT` file.
///
/// Encoding D pairs a 22-byte Variant A index with 8-byte records:
/// bytes 0-3 are intensity as unsigned 16.16 fixed point and bytes 4-7 are
/// a floating-point m/z word (see [`encoding_d_mz`]). The `_HEADER.TXT`
/// T1 polynomial applies to sqrt(m/z), which is proportional to flight time.
pub fn decode_encoding_d(scan_bytes: &[u8], params: &DecodeParams) -> crate::Result<Spectrum> {
    if scan_bytes.len() % 8 != 0 {
        return Err(crate::Error::Parse(format!(
            "Encoding D: scan size {} is not a multiple of 8",
            scan_bytes.len()
        )));
    }
    let n = scan_bytes.len() / 8;
    let mut out = Spectrum {
        mz: Vec::with_capacity(n),
        intensity: Vec::with_capacity(n),
    };
    for rec in scan_bytes.chunks_exact(8) {
        let intensity = u32::from_le_bytes([rec[0], rec[1], rec[2], rec[3]]);
        if intensity == 0 {
            continue;
        }
        let position = u32::from_le_bytes([rec[4], rec[5], rec[6], rec[7]]);
        let mz = encoding_d_mz(position)?;
        out.mz.push(params.cal.apply(mz.sqrt()).powi(2));
        out.intensity.push((f64::from(intensity) / 65536.0) as f32);
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

    // -- Encoding A helpers --

    /// 6-byte record with `count` ions at `mantissa * 2^(exponent - 24)`.
    fn enc_a_record(count: u16, exponent: u8, mantissa: u32) -> [u8; 6] {
        let mut r = [0u8; 6];
        r[0..2].copy_from_slice(&count.to_le_bytes());
        r[2] = exponent << 4;
        r[3..6].copy_from_slice(&mantissa.to_le_bytes()[..3]);
        r
    }

    // -- Encoding B/C helpers --

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

    // -- Encoding D tests --

    fn enc_d_record(intensity: u32, position: u32) -> [u8; 8] {
        let mut r = [0u8; 8];
        r[0..4].copy_from_slice(&intensity.to_le_bytes());
        r[4..8].copy_from_slice(&position.to_le_bytes());
        r
    }

    #[test]
    fn enc_d_position_word_is_floating_mz() {
        // Exponent 10, mantissa 0.5 exactly -> 512.
        assert_eq!(encoding_d_mz(0x5400_0000).unwrap(), 512.0);
        // Word observed at the Leu-Enk apex of a public Vion reference scan.
        let mz = encoding_d_mz(1_415_079_944).unwrap();
        assert!((mz - 556.20319).abs() < 1e-5, "{mz}");
        assert!(encoding_d_mz(0x5000_0000).is_err());
    }

    #[test]
    fn enc_d_decodes_fixed_point_intensity_and_skips_zeros() {
        let scan = bytes_of(&[
            enc_d_record(0, 0x5400_0000),
            enc_d_record((415 << 16) | 0x8000, 0x5400_0000),
            enc_d_record(0, 0x5800_0000),
        ]);
        let spec = decode_encoding_d(&scan, &test_params()).unwrap();
        assert_eq!(spec.mz.len(), 1);
        assert!((spec.mz[0] - 512.0).abs() < 1e-9);
        assert_eq!(spec.intensity, vec![415.5]);
    }

    #[test]
    fn enc_d_applies_calibration_to_sqrt_mz() {
        let mut params = test_params();
        params.cal = FunctionCal {
            coeffs: vec![0.0, 1.0001],
            cal_type: CalType::T1,
        };
        let scan = bytes_of(&[enc_d_record(1 << 16, 0x5400_0000)]);
        let spec = decode_encoding_d(&scan, &params).unwrap();
        assert!((spec.mz[0] - 512.0 * 1.0001f64.powi(2)).abs() < 1e-9);
    }

    // -- Encoding A tests --

    #[test]
    fn enc_a_decodes_floating_mz_and_count() {
        // Words from public PXD058812 scan: range ends 100 and 2000 Da.
        let scan = bytes_of(&[
            enc_a_record(0, 7, 0xC7_FFCA),
            enc_a_record(3, 8, 0x9F_A4F2),
            enc_a_record(0, 11, 0xF9_FF4A),
        ]);
        let spec = decode_encoding_a(&scan, &test_params()).unwrap();
        assert_eq!(spec.intensity, vec![3.0]);
        let expected = f64::from(0x9F_A4F2u32) * 2f64.powi(8 - 24);
        assert!((spec.mz[0] - expected).abs() < 1e-9, "mz={}", spec.mz[0]);
        assert!((encoding_a_mz(&enc_a_record(0, 7, 0xC7_FFCA)).unwrap() - 100.0).abs() < 1e-3);
        assert!((encoding_a_mz(&enc_a_record(0, 11, 0xF9_FF4A)).unwrap() - 2000.0).abs() < 0.05);
    }

    #[test]
    fn enc_a_count_uses_both_low_bytes() {
        let scan = bytes_of(&[enc_a_record(0x0102, 9, 0x80_0000)]);
        let spec = decode_encoding_a(&scan, &test_params()).unwrap();
        assert_eq!(spec.intensity, vec![258.0]);
    }

    #[test]
    fn enc_a_applies_calibration_to_sqrt_mz() {
        let mut params = test_params();
        params.cal = FunctionCal {
            coeffs: vec![0.0, 1.0001],
            cal_type: CalType::T1,
        };
        let scan = bytes_of(&[enc_a_record(1, 10, 0x80_0000)]);
        let spec = decode_encoding_a(&scan, &params).unwrap();
        assert!((spec.mz[0] - 512.0 * 1.0001f64.powi(2)).abs() < 1e-9);
    }

    #[test]
    fn enc_a_empty_bytes_is_empty() {
        let spec = decode_encoding_a(&[], &test_params()).unwrap();
        assert!(spec.mz.is_empty());
    }

    #[test]
    fn enc_a_rejects_unnormalized_or_flagged_words() {
        let unnormalized = bytes_of(&[enc_a_record(1, 9, 0x40_0000)]);
        assert!(decode_encoding_a(&unnormalized, &test_params()).is_err());
        let mut flagged = enc_a_record(1, 9, 0x80_0000);
        flagged[2] |= 0x01;
        assert!(decode_encoding_a(&bytes_of(&[flagged]), &test_params()).is_err());
    }

    #[test]
    fn enc_a_bad_size_is_error() {
        let data = vec![0u8; 7]; // not multiple of 6
        assert!(decode_encoding_a(&data, &test_params()).is_err());
    }

    // -- Encoding B tests --

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

    // -- Encoding C tests --

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

    // -- Corpus integration tests --
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
            [scan3.dat_offset as usize..(scan3.dat_offset + scan3.n_records * 6) as usize];
        let spec = decode_encoding_a(scan_bytes, &params).unwrap();

        assert!(!spec.mz.is_empty(), "scan 3 should have peaks");
        // Calibration moves peaks by well under 1%, so every decoded peak stays
        // inside the declared acquisition range.
        for &m in &spec.mz {
            assert!(m >= params.mz_low * 0.99, "mz={m} below mz_low");
            assert!(m <= params.mz_high * 1.01, "mz={m} above mz_high");
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
