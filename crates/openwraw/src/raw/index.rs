// Parser for _FUNCnnn.IDX - fixed-size binary index files, one per function.
// Each record in the index stores the byte offset and length of the
// corresponding spectrum in the paired _FUNCnnn.DAT file, along with
// retention time and other per-scan metadata.

use std::path::Path;

/// Record stride for Variant A (non-IMS / simple TOF-MS).
pub const STRIDE_A: usize = 22;
/// Record stride for Variant B (SYNAPT G2-S and later, Xevo G2-XS and G3).
pub const STRIDE_B: usize = 30;

/// Constant upper-16-bit marker in the Variant A +0x04 packed field.
/// Value 0x1800 identifies the DAT encoding format for this scan.
pub const VARIANT_A_TYPE_MARKER: u32 = 0x1800;

/// A scan index record from a Variant A (`_FUNCnnn.IDX`, 22-byte stride).
///
/// Observed in non-IMS Q-TOF instruments (PXD058812).
#[derive(Debug, Clone)]
pub struct ScanIndexA {
    /// Byte offset of this scan's data within `_FUNCnnn.DAT`.
    pub dat_offset: u32,
    /// Number of DAT records in this scan. The packed field at +0x04 uses
    /// its high byte as a format marker and its low 24 bits as the count.
    pub n_records: u32,
    /// Retention time (minutes).
    pub retention_time_min: f32,
    /// Number of centroid peaks (0 for blank scans).
    pub peak_count: u16,
}

/// A scan index record from a Variant B (`_FUNCnnn.IDX`, 30-byte stride).
///
/// Observed in SYNAPT G2-S/G2-Si/XS and Xevo G2-XS/G3 bundles. The record
/// has no record count, so the DAT record width is judged from the data by
/// the reader.
#[derive(Debug, Clone)]
pub struct ScanIndexB {
    /// Byte offset of this scan's data within `_FUNCnnn.DAT`.
    ///
    /// Stored as a 64-bit little-endian value split across two u32 words:
    /// the low word at +0x16 and the high word at +0x1A. The high word is 0
    /// for DAT files under 4 GiB and non-zero past that boundary.
    pub dat_offset: u64,
    /// Retention time (minutes).
    pub retention_time_min: f32,
}

/// Parsed index from a `_FUNCnnn.IDX` file.
#[derive(Debug, Clone)]
pub enum ScanIndex {
    A(Vec<ScanIndexA>),
    B(Vec<ScanIndexB>),
}

impl ScanIndex {
    /// Read and parse a `_FUNCnnn.IDX` file.
    ///
    /// The variant is detected from the file size: a size that is an exact
    /// multiple of 22 is Variant A; a size that is an exact multiple of 30
    /// (and not 22) is Variant B.  Returns an error if neither fits.
    pub fn from_path(path: &Path) -> crate::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Parse from a raw byte slice.
    pub fn from_bytes(data: &[u8]) -> crate::Result<Self> {
        let len = data.len();
        let fits_a = len % STRIDE_A == 0;
        let fits_b = len % STRIDE_B == 0;

        match (fits_a, fits_b) {
            (true, false) => Ok(ScanIndex::A(parse_variant_a(data)?)),
            (false, true) => Ok(ScanIndex::B(parse_variant_b(data)?)),
            (true, true) if len == 0 => Ok(ScanIndex::A(Vec::new())),
            (true, true) => {
                // At a 330-byte boundary, use the Variant A packed marker and
                // monotonically increasing DAT offsets to disambiguate.
                let looks_a = data.chunks_exact(STRIDE_A).all(|rec| {
                    let packed = u32::from_le_bytes([rec[4], rec[5], rec[6], rec[7]]);
                    // 0x18: existing 6/8-byte families; 0x4c/0x08: public
                    // LCT Premier mass/lock functions (12-byte records).
                    matches!(
                        packed & 0xff00_0000,
                        0x1800_0000 | 0x4c00_0000 | 0x0800_0000
                    )
                }) && data
                    .chunks_exact(STRIDE_A)
                    .map(|rec| u32::from_le_bytes([rec[0], rec[1], rec[2], rec[3]]))
                    .is_sorted();
                log::debug!(
                    "_FUNCnnn.IDX: {len} bytes fits both strides; record markers select {}",
                    if looks_a {
                        "22-byte Variant A"
                    } else {
                        "30-byte Variant B"
                    }
                );
                if looks_a {
                    Ok(ScanIndex::A(parse_variant_a(data)?))
                } else {
                    Ok(ScanIndex::B(parse_variant_b(data)?))
                }
            }
            (false, false) => Err(crate::Error::Parse(format!(
                "_FUNCnnn.IDX: size {len} is not a multiple of {STRIDE_A} (Variant A) \
                 or {STRIDE_B} (Variant B)"
            ))),
        }
    }

    /// Number of scan records in this index.
    pub fn len(&self) -> usize {
        match self {
            ScanIndex::A(v) => v.len(),
            ScanIndex::B(v) => v.len(),
        }
    }

    /// Returns `true` if the index contains no scan records.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn parse_variant_a(data: &[u8]) -> crate::Result<Vec<ScanIndexA>> {
    let n = data.len() / STRIDE_A;
    let mut records = Vec::with_capacity(n);

    for i in 0..n {
        let off = i * STRIDE_A;
        let rec = &data[off..off + STRIDE_A];

        let dat_offset = crate::bytes::read_u32_le(rec, 0x00)?;
        let packed = crate::bytes::read_u32_le(rec, 0x04)?;
        let n_records = packed & 0x00FF_FFFF;
        let retention_time_min = crate::bytes::read_f32_le(rec, 0x0C)?;
        let peak_count = crate::bytes::read_u16_le(rec, 0x10)?;

        records.push(ScanIndexA {
            dat_offset,
            n_records,
            retention_time_min,
            peak_count,
        });
    }

    Ok(records)
}

fn parse_variant_b(data: &[u8]) -> crate::Result<Vec<ScanIndexB>> {
    let n = data.len() / STRIDE_B;
    let mut records = Vec::with_capacity(n);

    for i in 0..n {
        let off = i * STRIDE_B;
        let rec = &data[off..off + STRIDE_B];

        let retention_time_min = crate::bytes::read_f32_le(rec, 0x0C)?;
        // +0x16 is the low word and +0x1A the high word of a 64-bit offset.
        // The high word is non-zero once a function's DAT exceeds 4 GiB.
        let lo = crate::bytes::read_u32_le(rec, 0x16)?;
        let hi = crate::bytes::read_u32_le(rec, 0x1A)?;
        let dat_offset = (u64::from(hi) << 32) | u64::from(lo);

        records.push(ScanIndexB {
            dat_offset,
            retention_time_min,
        });
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- helpers ---

    fn make_a_record(dat_off: u32, n_recs: u32, rt: f32, peaks: u16) -> [u8; STRIDE_A] {
        let mut rec = [0u8; STRIDE_A];
        rec[0x00..0x04].copy_from_slice(&dat_off.to_le_bytes());
        let packed: u32 = ((VARIANT_A_TYPE_MARKER) << 16) | n_recs;
        rec[0x04..0x08].copy_from_slice(&packed.to_le_bytes());
        rec[0x0C..0x10].copy_from_slice(&rt.to_le_bytes());
        rec[0x10..0x12].copy_from_slice(&peaks.to_le_bytes());
        rec
    }

    fn make_b_record(dat_off: u64, rt: f32) -> [u8; STRIDE_B] {
        let mut rec = [0u8; STRIDE_B];
        rec[0x0C..0x10].copy_from_slice(&rt.to_le_bytes());
        rec[0x16..0x1E].copy_from_slice(&dat_off.to_le_bytes());
        rec
    }

    fn a_data(records: &[[u8; STRIDE_A]]) -> Vec<u8> {
        records.iter().flat_map(|r| r.iter().copied()).collect()
    }

    #[test]
    fn variant_a_count_uses_all_twenty_four_bits() {
        let data = a_data(&[make_a_record(0, 110_646, 0.0, 0)]);
        let ScanIndex::A(records) = ScanIndex::from_bytes(&data).unwrap() else {
            panic!("expected Variant A");
        };
        assert_eq!(records[0].n_records, 110_646);
    }

    #[test]
    fn ambiguous_size_uses_record_markers() {
        let a: Vec<_> = (0..15)
            .map(|i| make_a_record(i * 6, 1, i as f32, 0))
            .collect();
        assert!(matches!(
            ScanIndex::from_bytes(&a_data(&a)).unwrap(),
            ScanIndex::A(_)
        ));
        let b: Vec<_> = (0..11).map(|i| make_b_record(i * 8, i as f32)).collect();
        assert!(matches!(
            ScanIndex::from_bytes(&b_data(&b)).unwrap(),
            ScanIndex::B(_)
        ));
    }

    #[test]
    fn ambiguous_size_recognizes_public_lct_markers() {
        // Both markers occur in the original MTBLS701/MTBLS13770 indexes.
        // Fifteen Variant A scans also fit eleven Variant B records.
        for marker in [0x4c00_0000u32, 0x0800_0000u32] {
            let a: Vec<_> = (0..15)
                .map(|i| {
                    let mut rec = make_a_record(i * 12, 1, i as f32, 0);
                    rec[4..8].copy_from_slice(&(marker | 1).to_le_bytes());
                    rec
                })
                .collect();
            let ScanIndex::A(records) = ScanIndex::from_bytes(&a_data(&a)).unwrap() else {
                panic!("expected Variant A for marker {marker:#010x}");
            };
            assert_eq!(records.len(), 15);
            assert!(records.iter().all(|r| r.n_records == 1));
        }
    }

    fn b_data(records: &[[u8; STRIDE_B]]) -> Vec<u8> {
        records.iter().flat_map(|r| r.iter().copied()).collect()
    }

    // --- Variant A ---

    #[test]
    fn parse_variant_a_blank_scan() {
        // Scans 0-2 in PXD058812 are blank: n_records=2, peaks=0, rt>0
        let data = a_data(&[make_a_record(0x00000000, 2, 0.0273, 0)]);
        let idx = ScanIndex::from_bytes(&data).unwrap();
        let ScanIndex::A(recs) = idx else {
            panic!("expected Variant A")
        };
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].dat_offset, 0);
        assert_eq!(recs[0].n_records, 2);
        assert!((recs[0].retention_time_min - 0.0273).abs() < 1e-4);
        assert_eq!(recs[0].peak_count, 0);
    }

    #[test]
    fn parse_variant_a_data_scan() {
        let data = a_data(&[make_a_record(0x00000024, 1050, 0.0829, 17)]);
        let ScanIndex::A(recs) = ScanIndex::from_bytes(&data).unwrap() else {
            panic!("expected Variant A")
        };
        assert_eq!(recs[0].dat_offset, 0x24);
        assert_eq!(recs[0].n_records, 1050);
        assert!((recs[0].retention_time_min - 0.0829).abs() < 1e-4);
        assert_eq!(recs[0].peak_count, 17);
    }

    #[test]
    fn parse_variant_a_multiple_records() {
        let data = a_data(&[
            make_a_record(0x00000000, 2, 0.0273, 0),
            make_a_record(0x0000000c, 2, 0.0458, 0),
            make_a_record(0x00000018, 2, 0.0644, 0),
            make_a_record(0x00000024, 1050, 0.0829, 17),
        ]);
        let ScanIndex::A(recs) = ScanIndex::from_bytes(&data).unwrap() else {
            panic!("expected Variant A")
        };
        assert_eq!(recs.len(), 4);
        assert_eq!(recs[3].dat_offset, 0x24);
        assert_eq!(recs[3].n_records, 1050);
    }

    #[test]
    fn variant_a_rt_is_monotonically_increasing() {
        // Retention times must increase across the run.
        let rts = [0.0273f32, 0.0458, 0.0644, 0.0829];
        let data = a_data(&rts.map(|rt| make_a_record(0, 2, rt, 0)));
        let ScanIndex::A(recs) = ScanIndex::from_bytes(&data).unwrap() else {
            panic!("expected Variant A")
        };
        for w in recs.windows(2) {
            assert!(
                w[1].retention_time_min > w[0].retention_time_min,
                "RT not monotonic: {} <= {}",
                w[1].retention_time_min,
                w[0].retention_time_min
            );
        }
    }

    // --- Variant B ---

    #[test]
    fn parse_variant_b_record() {
        let data = b_data(&[make_b_record(0x000047c8, 0.0540)]);
        let ScanIndex::B(recs) = ScanIndex::from_bytes(&data).unwrap() else {
            panic!("expected Variant B")
        };
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].dat_offset, 0x47c8);
        assert!((recs[0].retention_time_min - 0.0540).abs() < 1e-4);
    }

    #[test]
    fn parse_variant_b_multiple_records() {
        // Match first 4 records from PXD075602/DHPR_11257-1.raw/_FUNC001.IDX
        let data = b_data(&[
            make_b_record(0x00000000, 0.0368),
            make_b_record(0x000047c8, 0.0540),
            make_b_record(0x00008768, 0.0711),
            make_b_record(0x0000ccb8, 0.0883),
        ]);
        let ScanIndex::B(recs) = ScanIndex::from_bytes(&data).unwrap() else {
            panic!("expected Variant B")
        };
        assert_eq!(recs.len(), 4);
        assert_eq!(recs[0].dat_offset, 0x00000000);
        assert_eq!(recs[3].dat_offset, 0x0000ccb8);
    }

    #[test]
    fn variant_b_dat_offsets_increase() {
        let offsets = [0x00000000u64, 0x000047c8, 0x00008768, 0x0000ccb8];
        let data = b_data(&offsets.map(|o| make_b_record(o, 0.1)));
        let ScanIndex::B(recs) = ScanIndex::from_bytes(&data).unwrap() else {
            panic!("expected Variant B")
        };
        for w in recs.windows(2) {
            assert!(w[1].dat_offset > w[0].dat_offset);
        }
    }

    // Regression: a function whose DAT exceeds 4 GiB stores the high 32 bits
    // of the offset at +0x1A. Values mirror PXD045625 Abu_190520_Sha11.raw
    // _FUNC001.IDX scan 2699 (lo = 1,626,312, hi = 1), written byte by byte
    // so the test does not depend on the helper's layout.
    #[test]
    fn variant_b_offset_reads_high_word_at_0x1a() {
        let mut below = [0u8; STRIDE_B];
        below[0x16..0x1A].copy_from_slice(&0xFFFF_FF00u32.to_le_bytes());
        let mut above = [0u8; STRIDE_B];
        above[0x16..0x1A].copy_from_slice(&1_626_312u32.to_le_bytes());
        above[0x1A..0x1E].copy_from_slice(&1u32.to_le_bytes());
        let ScanIndex::B(recs) = ScanIndex::from_bytes(&b_data(&[below, above])).unwrap() else {
            panic!("expected Variant B")
        };
        assert_eq!(recs[0].dat_offset, 0xFFFF_FF00);
        assert_eq!(recs[1].dat_offset, (1u64 << 32) + 1_626_312);
        assert!(recs[1].dat_offset > recs[0].dat_offset);
    }

    // --- Detection ---

    #[test]
    fn empty_file_is_variant_a() {
        let idx = ScanIndex::from_bytes(&[]).unwrap();
        assert!(matches!(idx, ScanIndex::A(_)));
        assert!(idx.is_empty());
    }

    #[test]
    fn bad_size_is_error() {
        let data = vec![0u8; 25]; // not divisible by 22 or 30
        let err = ScanIndex::from_bytes(&data).unwrap_err();
        assert!(err.to_string().contains("Variant A"));
    }

    #[test]
    fn len_matches_record_count() {
        let data = a_data(&[make_a_record(0, 2, 0.1, 0), make_a_record(12, 2, 0.2, 0)]);
        let idx = ScanIndex::from_bytes(&data).unwrap();
        assert_eq!(idx.len(), 2);
    }
}
