// Reader for _FUNCnnn.DAT - the binary spectrum data files.
// Spectra are stored contiguously, referenced by offsets from the
// paired .IDX file. The record layout is chosen per function by the reader
// (see `crate::reader::Encoding`).

use crate::raw::header::FunctionCal;

/// Parameters shared by the DAT decoders.
///
/// Every decoder reads an uncalibrated m/z from each record and applies the
/// function's `_HEADER.TXT` T1 calibration polynomial to sqrt(m/z).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DecodeParams {
    /// Per-function T1 calibration polynomial from `_HEADER.TXT`.
    pub cal: FunctionCal,
}

impl DecodeParams {
    /// Parameters for a function calibrated by `cal`.
    pub fn new(cal: FunctionCal) -> Self {
        Self { cal }
    }
}

/// One decoded scan: calibrated m/z (Da) and intensity per profile point.
///
/// No lock-mass correction is applied. Ion mobility is not decoded, so a
/// scan from a mobility acquisition is returned as a plain m/z spectrum.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct Spectrum {
    pub mz: Vec<f64>,
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
/// followed by a floating-point m/z word (see `encoding_a_mz`). Zero-count
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
/// Encoding D uses 8-byte records with either index variant (22-byte
/// Variant A or 30-byte Variant B): bytes 0-3 are intensity as unsigned
/// 16.16 fixed point and bytes 4-7 are a floating-point m/z word (see
/// `encoding_d_mz`). The `_HEADER.TXT` T1 polynomial applies to sqrt(m/z),
/// which is proportional to flight time.
///
/// Records with zero intensity are skipped. A position word without the
/// leading mantissa bit is an error, not a skipped record.
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

/// Record width (8 or 12 bytes) of one Variant B scan, judged from the
/// position words, or `None` when the scan does not decide it.
///
/// Variant B index records carry no record count, so the width is read from
/// the data: under the right width every record's bytes 4-7 are a position
/// word with the leading mantissa bit set and m/z never decreases within a
/// scan. Scans with fewer than three records under either width are not
/// used, since a short scan can fit both by chance.
pub(crate) fn variant_b_record_width(scan_bytes: &[u8]) -> Option<u64> {
    let fits = |width: usize| {
        if scan_bytes.len() % width != 0 || scan_bytes.len() / width < 3 {
            return false;
        }
        let mut previous = 0u32;
        scan_bytes.chunks_exact(width).all(|rec| {
            let position = u32::from_le_bytes([rec[4], rec[5], rec[6], rec[7]]);
            let ok = position & ENC_D_LEADING_ONE != 0 && position >= previous;
            previous = position;
            ok
        })
    };
    match (fits(8), fits(12)) {
        (true, false) => Some(8),
        (false, true) => Some(12),
        _ => None,
    }
}

// -- Encoding E --

/// Decode one scan slice from an Encoding E `_FUNCnnn.DAT` file.
///
/// Encoding E uses 12-byte records. It is observed with the 22-byte
/// Variant A index in public LCT Premier bundles and with the 30-byte
/// Variant B index in the lock-mass functions of some public Xevo G2-XS
/// bundles.
///
/// The intensity word has a 21-bit normalized mantissa and a 5-bit exponent
/// at bits 22-26: intensity = mantissa * 2^(exponent - 21). Higher bits are
/// flags, not part of the exponent. The position word at bytes 4-7 uses the
/// same floating-point m/z representation as Encoding D. Bytes 8-11 and
/// the intensity flags are not interpreted; flagged peaks remain present.
/// Derived from original MTBLS701 and MTBLS13770 bytes and scan-index TIC
/// self-consistency, without vendor software or vendor-derived output.
pub fn decode_encoding_e(scan_bytes: &[u8], params: &DecodeParams) -> crate::Result<Spectrum> {
    if scan_bytes.len() % 12 != 0 {
        return Err(crate::Error::Parse(format!(
            "Encoding E: scan size {} is not a multiple of 12",
            scan_bytes.len()
        )));
    }
    let n = scan_bytes.len() / 12;
    let mut out = Spectrum {
        mz: Vec::with_capacity(n),
        intensity: Vec::with_capacity(n),
    };
    for rec in scan_bytes.chunks_exact(12) {
        let word = u32::from_le_bytes([rec[0], rec[1], rec[2], rec[3]]);
        let mantissa = word & 0x001f_ffff;
        if mantissa == 0 {
            continue;
        }
        if mantissa & (1 << 20) == 0 || word & (1 << 21) != 0 {
            return Err(crate::Error::Parse(format!(
                "Encoding E: unsupported intensity word {word:#010x}"
            )));
        }
        let exponent = ((word >> 22) & 0x1f) as i32;
        let intensity = f64::from(mantissa) * 2f64.powi(exponent - 21);
        let position = u32::from_le_bytes([rec[4], rec[5], rec[6], rec[7]]);
        let mz = encoding_d_mz(position)?;
        out.mz.push(params.cal.apply(mz.sqrt()).powi(2));
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

    fn test_params() -> DecodeParams {
        DecodeParams::new(identity_cal())
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

    // -- Encoding D on the 30-byte (Variant B) index --

    /// Five consecutive records around the leucine enkephalin [M+H]+ apex of
    /// a lock-mass scan in public bundle PXD068881 (SYNAPT G2-Si, function 3,
    /// scan 4), as stored in `_FUNC003.DAT`.
    const SYNAPT_LOCK_RECORDS: [[u8; 8]; 5] = [
        [0x00, 0x64, 0xd6, 0x02, 0xc8, 0x7c, 0x58, 0x54],
        [0x00, 0x41, 0x55, 0x03, 0x08, 0x82, 0x58, 0x54],
        [0x00, 0xc2, 0x5f, 0x03, 0x50, 0x87, 0x58, 0x54],
        [0x00, 0xc5, 0x53, 0x03, 0x98, 0x8c, 0x58, 0x54],
        [0x00, 0xea, 0x12, 0x03, 0xd8, 0x91, 0x58, 0x54],
    ];

    /// `Cal Function 3` from the same bundle's `_HEADER.TXT`, digits as
    /// written there.
    #[allow(clippy::excessive_precision)]
    fn synapt_lock_cal() -> FunctionCal {
        FunctionCal {
            coeffs: vec![
                -5.493524164097924e-4,
                9.997320908477830e-1,
                2.699224919217800e-5,
                -9.288655145431219e-7,
                1.489803356847283e-8,
                -9.018736795357334e-11,
            ],
            cal_type: CalType::T1,
        }
    }

    #[test]
    fn enc_d_decodes_variant_b_records_with_fractional_intensity() {
        let scan = bytes_of(&SYNAPT_LOCK_RECORDS);
        let spec = decode_encoding_d(&scan, &test_params()).unwrap();
        assert_eq!(spec.mz.len(), 5);
        // Uncalibrated m/z steps by one ADC sample (about 0.0103 Da here).
        assert!((spec.mz[3] - 556.274_597).abs() < 1e-5, "{}", spec.mz[3]);
        assert!(spec.mz.windows(2).all(|w| w[1] > w[0]));
        // Bytes 0-1 are the fraction of a 16.16 intensity: 0x0353_c500.
        assert_eq!(spec.intensity[3], 851.769_53);
    }

    #[test]
    fn enc_d_variant_b_lock_apex_is_within_tens_of_ppm() {
        const LEU_ENK_MH: f64 = 556.2766;
        let scan = bytes_of(&SYNAPT_LOCK_RECORDS);
        let spec = decode_encoding_d(&scan, &DecodeParams::new(synapt_lock_cal())).unwrap();
        let ppm = (spec.mz[3] - LEU_ENK_MH) / LEU_ENK_MH * 1e6;
        assert!(
            ppm.abs() < 20.0,
            "apex record at {} ({ppm:.1} ppm)",
            spec.mz[3]
        );
    }

    #[test]
    fn variant_b_width_is_read_from_position_words() {
        assert_eq!(
            variant_b_record_width(&bytes_of(&SYNAPT_LOCK_RECORDS)),
            Some(8)
        );
        // 12-byte records from a public Xevo G2-XS lock-mass function
        // (PXD053170 20231113_NSE_Sample_High.raw, _FUNC002.DAT).
        let xevo_lock = bytes_of(&[
            enc_e_record(0x0290_f562, 0x5458_0198, 0x0488_6c2c),
            enc_e_record(0x0414_e82b, 0x5458_3200, 0x04b9_493f),
            enc_e_record(0x1557_d0aa, 0x5458_5190, 0x00de_07a9),
            enc_e_record(0x041e_92f9, 0x5458_70d0, 0x00bb_49a1),
        ]);
        assert_eq!(variant_b_record_width(&xevo_lock), Some(12));
        // Too short to decide, and decreasing m/z fits neither width.
        assert_eq!(
            variant_b_record_width(&bytes_of(&SYNAPT_LOCK_RECORDS[..2])),
            None
        );
        let mut reversed = SYNAPT_LOCK_RECORDS;
        reversed.reverse();
        assert_eq!(variant_b_record_width(&bytes_of(&reversed)), None);
        assert_eq!(variant_b_record_width(&[]), None);
    }

    #[test]
    fn enc_d_rejects_word_without_leading_bit_in_variant_b_scan() {
        let mut recs = SYNAPT_LOCK_RECORDS;
        recs[2][7] = 0x50; // clears bit 26 of the position word
        assert!(decode_encoding_d(&bytes_of(&recs), &test_params()).is_err());
    }

    // -- Encoding E tests --

    fn enc_e_record(intensity: u32, position: u32, auxiliary: u32) -> [u8; 12] {
        let mut r = [0u8; 12];
        r[0..4].copy_from_slice(&intensity.to_le_bytes());
        r[4..8].copy_from_slice(&position.to_le_bytes());
        r[8..12].copy_from_slice(&auxiliary.to_le_bytes());
        r
    }

    #[test]
    fn enc_e_decodes_compressed_intensity_without_flag_bits() {
        let scan = bytes_of(&[
            enc_e_record(0, 0, 0),
            enc_e_record(0x0050_0000, 0x5400_0000, 0),
            enc_e_record(0x1050_0000, 0x5400_0000, 0x0418_2c40),
            enc_e_record(0x3058_0000, 0x5400_0000, u32::MAX),
            enc_e_record(0x0090_0000, 0x5400_0000, 1),
        ]);
        let spec = decode_encoding_e(&scan, &test_params()).unwrap();
        assert!(spec.mz.iter().all(|mz| (*mz - 512.0).abs() < 1e-9));
        assert_eq!(spec.intensity, vec![1.0, 1.0, 1.5, 2.0]);
    }

    #[test]
    fn enc_e_applies_calibration_to_sqrt_mz() {
        let mut params = test_params();
        params.cal = FunctionCal {
            coeffs: vec![0.0, 1.0001],
            cal_type: CalType::T1,
        };
        let scan = bytes_of(&[enc_e_record(0x0050_0000, 0x5400_0000, 0)]);
        let spec = decode_encoding_e(&scan, &params).unwrap();
        assert!((spec.mz[0] - 512.0 * 1.0001f64.powi(2)).abs() < 1e-9);
    }

    #[test]
    fn enc_e_rejects_truncation_and_unsupported_words() {
        assert!(decode_encoding_e(&[0; 11], &test_params()).is_err());
        for word in [0x0048_0000, 0x0070_0000] {
            let scan = bytes_of(&[enc_e_record(word, 0x5400_0000, 0)]);
            assert!(decode_encoding_e(&scan, &test_params()).is_err());
        }
        let scan = bytes_of(&[enc_e_record(0x0050_0000, 0x5000_0000, 0)]);
        assert!(decode_encoding_e(&scan, &test_params()).is_err());
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

    // -- Corpus integration tests --
    // These tests read bundles under OPENWRAW_CORPUS and skip when absent
    // (fail instead with REQUIRE_CORPUS=1); see crate::test_corpus.

    #[test]
    fn corpus_encoding_a_pxd058812() {
        use crate::raw::{functions_inf::FunctionTable, index::ScanIndex};

        let Some(raw) = crate::test_corpus::bundle(&["PXD058812/molecular_mass_P15_01.raw"]) else {
            return;
        };

        let header = crate::raw::header::Header::from_path(&raw.join("_HEADER.TXT")).unwrap();
        let funcs = FunctionTable::from_path(&raw.join("_FUNCTNS.INF")).unwrap();
        let f = &funcs.functions[0];

        let params = DecodeParams::new(header.cal_functions[&1].clone());
        let (mz_low, mz_high) = (f64::from(f.mz_low), f64::from(f.mz_high));

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
            assert!(m >= mz_low * 0.99, "mz={m} below mz_low");
            assert!(m <= mz_high * 1.01, "mz={m} above mz_high");
        }
        for &i in &spec.intensity {
            assert!(i > 0.0, "zero intensity should have been filtered");
        }
    }
}
