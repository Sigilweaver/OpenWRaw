// Parser for _FUNCTNS.INF - the binary file describing each acquisition
// function (MS1 survey, MS2 DDA fragmentation, MRM channels, IMS, etc.).
// Each function record describes scan mode, polarity, mass range, and
// points to the corresponding _FUNCnnn.DAT / _FUNCnnn.IDX files.

use std::path::Path;

/// Size of one function record in bytes.
pub const RECORD_SIZE: usize = 416;

/// Scan subtype byte values observed in the corpus.
///
/// Bit 7 (0x80) flags a lock-mass / reference function.
pub mod subtype {
    pub const OLDER_QTOF_SURVEY: u8 = 0x25;
    pub const G2_SURVEY: u8 = 0x71;
    pub const G2_LOCKMASS: u8 = 0xf1; // 0x71 | 0x80
}

/// A single acquisition function record from `_FUNCTNS.INF`.
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    /// 1-based index of this function (position in the file + 1).
    pub index: u32,
    /// Raw function type code at +0x000. Always 0x12 in known corpus.
    pub function_type: u8,
    /// Scan subtype byte at +0x001. See `subtype` constants.
    pub scan_subtype: u8,
    /// Total slot duration per scan cycle (s), `scan_time + interscan_delay`.
    /// Stored at +0x002.
    pub cycle_time_s: f32,
    /// Idle time between end of one scan and start of the next (s). +0x01C.
    pub interscan_delay_s: f32,
    /// Data collection time per scan (s). +0x020.
    pub scan_time_s: f32,
    /// Number of TDC bins per pusher pulse. +0x010.
    pub tof_depth: u16,
    /// Acquisition m/z lower bound (Da). +0x0A0.
    pub mz_low: f32,
    /// Acquisition m/z upper bound (Da). +0x120.
    pub mz_high: f32,
}

impl FunctionInfo {
    /// Returns `true` if this function is a lock-mass / reference channel.
    ///
    /// Identified by bit 7 of `scan_subtype` (observed value 0xf1 = 0x71 | 0x80).
    pub fn is_lock_mass(&self) -> bool {
        self.scan_subtype & 0x80 != 0
    }
}

/// Parsed contents of a `_FUNCTNS.INF` file.
#[derive(Debug, Clone)]
pub struct FunctionTable {
    pub functions: Vec<FunctionInfo>,
}

impl FunctionTable {
    /// Read and parse a `_FUNCTNS.INF` file.
    pub fn from_path(path: &Path) -> crate::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Parse from a raw byte slice.
    pub fn from_bytes(data: &[u8]) -> crate::Result<Self> {
        if data.len() % RECORD_SIZE != 0 {
            return Err(crate::Error::Parse(format!(
                "_FUNCTNS.INF: file size {} is not a multiple of {} (record size)",
                data.len(),
                RECORD_SIZE
            )));
        }

        let n = data.len() / RECORD_SIZE;
        let mut functions = Vec::with_capacity(n);

        for i in 0..n {
            let off = i * RECORD_SIZE;
            let rec = &data[off..off + RECORD_SIZE];

            let function_type = rec[0x000];
            let scan_subtype = rec[0x001];
            let cycle_time_s = f32::from_le_bytes(rec[0x002..0x006].try_into().unwrap());
            let interscan_delay_s = f32::from_le_bytes(rec[0x01C..0x020].try_into().unwrap());
            let scan_time_s = f32::from_le_bytes(rec[0x020..0x024].try_into().unwrap());
            let tof_depth = u16::from_le_bytes(rec[0x010..0x012].try_into().unwrap());
            let mz_low = f32::from_le_bytes(rec[0x0A0..0x0A4].try_into().unwrap());
            let mz_high = f32::from_le_bytes(rec[0x120..0x124].try_into().unwrap());

            functions.push(FunctionInfo {
                index: (i + 1) as u32,
                function_type,
                scan_subtype,
                cycle_time_s,
                interscan_delay_s,
                scan_time_s,
                tof_depth,
                mz_low,
                mz_high,
            });
        }

        Ok(FunctionTable { functions })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Construct a minimal 416-byte record matching PXD058812/molecular_mass_P15_01.raw.
    // function_type=0x12, scan_subtype=0x25, cycle=1.1, inter=0.1,
    // tof_depth=17204, scan=1.0, mz_low=100, mz_high=2000.
    #[allow(clippy::too_many_arguments)]
    fn make_record(
        func_type: u8,
        subtype: u8,
        cycle_s: f32,
        inter_s: f32,
        tof_depth: u16,
        scan_s: f32,
        mz_low: f32,
        mz_high: f32,
    ) -> Vec<u8> {
        let mut rec = vec![0u8; RECORD_SIZE];
        rec[0x000] = func_type;
        rec[0x001] = subtype;
        rec[0x002..0x006].copy_from_slice(&cycle_s.to_le_bytes());
        rec[0x006..0x00A].copy_from_slice(&inter_s.to_le_bytes()); // duplicate at +0x006
        rec[0x010..0x012].copy_from_slice(&tof_depth.to_le_bytes());
        rec[0x01C..0x020].copy_from_slice(&inter_s.to_le_bytes());
        rec[0x020..0x024].copy_from_slice(&scan_s.to_le_bytes());
        rec[0x0A0..0x0A4].copy_from_slice(&mz_low.to_le_bytes());
        rec[0x120..0x124].copy_from_slice(&mz_high.to_le_bytes());
        rec
    }

    fn p15_record() -> Vec<u8> {
        make_record(0x12, 0x25, 1.1, 0.1, 17204, 1.0, 100.0, 2000.0)
    }

    fn g2_survey_record() -> Vec<u8> {
        make_record(0x12, 0x71, 0.314, 0.014, 16704, 0.3, 100.0, 2000.0)
    }

    fn g2_lockmass_record() -> Vec<u8> {
        make_record(0x12, 0xf1, 0.4, 0.1, 16704, 0.3, 100.0, 2000.0)
    }

    #[test]
    fn parse_single_function() {
        let data = p15_record();
        let table = FunctionTable::from_bytes(&data).unwrap();
        assert_eq!(table.functions.len(), 1);
        let f = &table.functions[0];
        assert_eq!(f.index, 1);
        assert_eq!(f.function_type, 0x12);
        assert_eq!(f.scan_subtype, 0x25);
        assert!((f.cycle_time_s - 1.1).abs() < 1e-5);
        assert!((f.interscan_delay_s - 0.1).abs() < 1e-5);
        assert!((f.scan_time_s - 1.0).abs() < 1e-5);
        assert_eq!(f.tof_depth, 17204);
        assert!((f.mz_low - 100.0).abs() < 1e-3);
        assert!((f.mz_high - 2000.0).abs() < 1e-3);
    }

    #[test]
    fn parse_three_functions() {
        let mut data = g2_survey_record();
        data.extend_from_slice(&g2_survey_record());
        data.extend_from_slice(&g2_lockmass_record());
        let table = FunctionTable::from_bytes(&data).unwrap();
        assert_eq!(table.functions.len(), 3);
        assert_eq!(table.functions[0].index, 1);
        assert_eq!(table.functions[1].index, 2);
        assert_eq!(table.functions[2].index, 3);
    }

    #[test]
    fn is_lock_mass_flag() {
        let mut data = g2_survey_record();
        data.extend_from_slice(&g2_lockmass_record());
        let table = FunctionTable::from_bytes(&data).unwrap();
        assert!(!table.functions[0].is_lock_mass());
        assert!(table.functions[1].is_lock_mass());
    }

    #[test]
    fn wrong_file_size_is_error() {
        let data = vec![0u8; 417]; // not a multiple of 416
        let err = FunctionTable::from_bytes(&data).unwrap_err();
        assert!(err.to_string().contains("record size"));
    }

    #[test]
    fn empty_file_gives_empty_table() {
        let table = FunctionTable::from_bytes(&[]).unwrap();
        assert!(table.functions.is_empty());
    }

    #[test]
    fn corpus_pxd058812_values() {
        // Exact binary values from molecular_mass_P15_01.raw/_FUNCTNS.INF:
        // cycle=1.10, inter=0.10, tof_depth=17204, scan=1.00, mz=[100,2000]
        let data = p15_record();
        let f = &FunctionTable::from_bytes(&data).unwrap().functions[0];
        assert!(
            (f.cycle_time_s - (f.scan_time_s + f.interscan_delay_s)).abs() < 1e-4,
            "cycle_time should equal scan_time + interscan_delay"
        );
    }

    #[test]
    fn corpus_ctpa_values() {
        // 3 functions: 2 survey (0x71) + 1 lock-mass (0xf1)
        // scan=0.3, inter=0.014, tof_depth=16704
        let mut data = g2_survey_record();
        data.extend_from_slice(&g2_survey_record());
        data.extend_from_slice(&g2_lockmass_record());
        let table = FunctionTable::from_bytes(&data).unwrap();
        for f in &table.functions[0..2] {
            assert!((f.scan_time_s - 0.3).abs() < 1e-4);
            assert!((f.interscan_delay_s - 0.014).abs() < 1e-4);
            assert_eq!(f.tof_depth, 16704);
        }
    }
}
