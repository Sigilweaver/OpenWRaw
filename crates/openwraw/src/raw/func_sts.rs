// Parser for _FUNCnnn.STS - the binary per-scan statistics file present
// alongside each _FUNCnnn.DAT/_FUNCnnn.IDX pair. Records per-scan instrument
// housekeeping values (electrode voltages, collision energies, push counts,
// lock-mass corrections, TIC traces) as a table of named channels.
//
// Layout (see docs/docs/format/07-func-sts.md):
//   [32-byte file preamble]
//   [n_desc x 48-byte descriptor records]   (starting at offset 0x20)
//   [n_scans x scan_record_size bytes of per-scan data]

use std::path::Path;

const PREAMBLE_SIZE: usize = 32;
const DESCRIPTOR_SIZE: usize = 48;
/// Byte length of the null-padded ASCII channel name within a descriptor.
const NAME_LEN: usize = DESCRIPTOR_SIZE - 6;

/// One channel's encoding, as recorded in a descriptor record's type field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelEncoding {
    U8,
    I16,
    U32,
    F32,
}

impl ChannelEncoding {
    fn from_code(code: u16) -> crate::Result<Self> {
        match code {
            0 => Ok(Self::U8),
            1 => Ok(Self::I16),
            2 => Ok(Self::U32),
            3 => Ok(Self::F32),
            other => Err(crate::Error::Parse(format!(
                "_FUNCnnn.STS: unknown channel encoding type {other}"
            ))),
        }
    }
}

/// One channel descriptor: name, encoding, and byte offset within each
/// per-scan record.
#[derive(Debug, Clone)]
pub struct ChannelDescriptor {
    /// Channel sequence number (not always contiguous; some skip).
    pub seq: u16,
    pub encoding: ChannelEncoding,
    /// Byte offset of this channel's value within each scan record.
    pub offset: usize,
    /// Null-padded ASCII channel name, trimmed of trailing NULs.
    pub name: String,
}

/// Parsed contents of a `_FUNCnnn.STS` file: channel descriptors plus the
/// raw per-scan data section, ready for by-name, by-scan lookups.
#[derive(Debug, Clone)]
pub struct FuncSts {
    scan_record_size: usize,
    n_scans: usize,
    descriptors: Vec<ChannelDescriptor>,
    data: Vec<u8>,
}

impl FuncSts {
    /// Read and parse a `_FUNCnnn.STS` file at the given path.
    pub fn from_path(path: &Path) -> crate::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Parse from an in-memory byte slice (useful for testing).
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
        if bytes.len() < PREAMBLE_SIZE {
            return Err(crate::Error::Parse(format!(
                "_FUNCnnn.STS too small: {} bytes (need at least {PREAMBLE_SIZE})",
                bytes.len()
            )));
        }

        let data_offset = crate::bytes::read_u16_le(bytes, 0x00)? as usize;
        let scan_record_size = crate::bytes::read_u16_le(bytes, 0x04)? as usize;
        let n_desc = crate::bytes::read_u16_le(bytes, 0x06)? as usize;

        let expected_data_offset = PREAMBLE_SIZE + n_desc * DESCRIPTOR_SIZE;
        if data_offset != expected_data_offset {
            return Err(crate::Error::Parse(format!(
                "_FUNCnnn.STS: data_offset {data_offset} does not match \
                 preamble + n_desc*48 = {expected_data_offset}"
            )));
        }
        if bytes.len() < data_offset {
            return Err(crate::Error::Parse(format!(
                "_FUNCnnn.STS: file too small for declared {n_desc} descriptor records"
            )));
        }

        let mut descriptors = Vec::with_capacity(n_desc);
        for i in 0..n_desc {
            let off = PREAMBLE_SIZE + i * DESCRIPTOR_SIZE;
            let rec = &bytes[off..off + DESCRIPTOR_SIZE];
            let seq = crate::bytes::read_u16_le(rec, 0x00)?;
            let encoding = ChannelEncoding::from_code(crate::bytes::read_u16_le(rec, 0x02)?)?;
            let offset = crate::bytes::read_u16_le(rec, 0x04)? as usize;
            let name_bytes = &rec[0x06..0x06 + NAME_LEN];
            let name_end = name_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(name_bytes.len());
            let name = String::from_utf8_lossy(&name_bytes[..name_end])
                .trim()
                .to_owned();
            descriptors.push(ChannelDescriptor {
                seq,
                encoding,
                offset,
                name,
            });
        }

        if scan_record_size == 0 {
            return Err(crate::Error::Parse(
                "_FUNCnnn.STS: scan_record_size is zero".to_owned(),
            ));
        }
        let remaining = bytes.len() - data_offset;
        let n_scans = remaining / scan_record_size;
        let data = bytes[data_offset..data_offset + n_scans * scan_record_size].to_vec();

        Ok(FuncSts {
            scan_record_size,
            n_scans,
            descriptors,
            data,
        })
    }

    /// Number of scans covered by the per-scan data section.
    pub fn scan_count(&self) -> usize {
        self.n_scans
    }

    /// Look up a channel descriptor by exact (trimmed) name.
    pub fn channel(&self, name: &str) -> Option<&ChannelDescriptor> {
        self.descriptors.iter().find(|d| d.name == name)
    }

    /// Decode one channel's value for a given 0-based scan index.
    ///
    /// Returns `None` if `scan_idx` is out of range for the parsed data
    /// section; corrupt/truncated numeric reads are surfaced as `None` too,
    /// since a housekeeping channel failing to decode shouldn't abort
    /// spectrum construction (unlike the mass-defining primary DAT/IDX
    /// parsers, which do return `Result`).
    pub fn value_at(&self, desc: &ChannelDescriptor, scan_idx: usize) -> Option<f64> {
        if scan_idx >= self.n_scans {
            return None;
        }
        let rec_start = scan_idx * self.scan_record_size;
        let rec = &self.data[rec_start..rec_start + self.scan_record_size];
        match desc.encoding {
            ChannelEncoding::U8 => rec.get(desc.offset).map(|&b| b as f64),
            ChannelEncoding::I16 => crate::bytes::read_i16_le(rec, desc.offset)
                .ok()
                .map(|v| v as f64),
            ChannelEncoding::U32 => crate::bytes::read_u32_le(rec, desc.offset)
                .ok()
                .map(|v| v as f64),
            ChannelEncoding::F32 => crate::bytes::read_f32_le(rec, desc.offset)
                .ok()
                .map(|v| v as f64),
        }
    }

    /// Per-scan collision energy (eV), if this function's STS file records
    /// a "Collision Energy" channel.
    pub fn collision_energy(&self, scan_idx: usize) -> Option<f64> {
        let desc = self.channel("Collision Energy")?;
        self.value_at(desc, scan_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_preamble(data_offset: u16, scan_record_size: u16, n_desc: u16) -> Vec<u8> {
        let mut p = vec![0u8; PREAMBLE_SIZE];
        p[0x00..0x02].copy_from_slice(&data_offset.to_le_bytes());
        p[0x02..0x04].copy_from_slice(&1u16.to_le_bytes()); // version
        p[0x04..0x06].copy_from_slice(&scan_record_size.to_le_bytes());
        p[0x06..0x08].copy_from_slice(&n_desc.to_le_bytes());
        p
    }

    fn make_descriptor(seq: u16, encoding: u16, offset: u16, name: &str) -> Vec<u8> {
        let mut d = vec![0u8; DESCRIPTOR_SIZE];
        d[0x00..0x02].copy_from_slice(&seq.to_le_bytes());
        d[0x02..0x04].copy_from_slice(&encoding.to_le_bytes());
        d[0x04..0x06].copy_from_slice(&offset.to_le_bytes());
        let name_bytes = name.as_bytes();
        let n = name_bytes.len().min(NAME_LEN);
        d[0x06..0x06 + n].copy_from_slice(&name_bytes[..n]);
        d
    }

    #[test]
    fn parse_single_channel_f32() {
        // One descriptor: "Collision Energy" (f32) at record offset 0.
        // scan_record_size = 4, two scans: 10.0 eV, 20.0 eV.
        let mut bytes = make_preamble(32 + 48, 4, 1);
        bytes.extend(make_descriptor(62, 3, 0, "Collision Energy"));
        bytes.extend(10.0f32.to_le_bytes());
        bytes.extend(20.0f32.to_le_bytes());

        let sts = FuncSts::from_bytes(&bytes).unwrap();
        assert_eq!(sts.scan_count(), 2);
        let ce = sts.channel("Collision Energy").expect("channel missing");
        assert_eq!(ce.seq, 62);
        assert!((sts.value_at(ce, 0).unwrap() - 10.0).abs() < 1e-6);
        assert!((sts.value_at(ce, 1).unwrap() - 20.0).abs() < 1e-6);
    }

    #[test]
    fn collision_energy_convenience_accessor() {
        let mut bytes = make_preamble(32 + 48, 4, 1);
        bytes.extend(make_descriptor(62, 3, 0, "Collision Energy"));
        bytes.extend(4.0f32.to_le_bytes());

        let sts = FuncSts::from_bytes(&bytes).unwrap();
        assert!((sts.collision_energy(0).unwrap() - 4.0).abs() < 1e-6);
    }

    #[test]
    fn missing_channel_returns_none() {
        let mut bytes = make_preamble(32 + 48, 4, 1);
        bytes.extend(make_descriptor(52, 1, 0, "Cone"));
        bytes.extend(30i16.to_le_bytes());
        bytes.extend(0u16.to_le_bytes()); // pad to 4-byte record

        let sts = FuncSts::from_bytes(&bytes).unwrap();
        assert!(sts.collision_energy(0).is_none());
    }

    #[test]
    fn scan_idx_out_of_range_returns_none() {
        let mut bytes = make_preamble(32 + 48, 4, 1);
        bytes.extend(make_descriptor(62, 3, 0, "Collision Energy"));
        bytes.extend(4.0f32.to_le_bytes());

        let sts = FuncSts::from_bytes(&bytes).unwrap();
        let ce = sts.channel("Collision Energy").unwrap();
        assert!(sts.value_at(ce, 5).is_none());
    }

    #[test]
    fn multiple_channels_mixed_encodings() {
        // scan_record_size = 7: u8 Segment(0), i16 Cone(1), f32 CE(3).
        let mut bytes = make_preamble(32 + 48 * 3, 7, 3);
        bytes.extend(make_descriptor(13, 0, 0, "Segment Number"));
        bytes.extend(make_descriptor(52, 1, 1, "Cone"));
        bytes.extend(make_descriptor(62, 3, 3, "Collision Energy"));
        bytes.push(0u8); // Segment Number
        bytes.extend(30i16.to_le_bytes()); // Cone
        bytes.extend(6.5f32.to_le_bytes()); // Collision Energy

        let sts = FuncSts::from_bytes(&bytes).unwrap();
        assert_eq!(sts.scan_count(), 1);
        let cone = sts.channel("Cone").unwrap();
        assert!((sts.value_at(cone, 0).unwrap() - 30.0).abs() < 1e-6);
        assert!((sts.collision_energy(0).unwrap() - 6.5).abs() < 1e-6);
    }

    #[test]
    fn data_offset_mismatch_is_error() {
        // Claims data_offset=100 but preamble+1desc*48 = 80.
        let mut bytes = make_preamble(100, 4, 1);
        bytes.extend(make_descriptor(62, 3, 0, "Collision Energy"));
        bytes.extend(vec![0u8; 100 - bytes.len()]);
        assert!(FuncSts::from_bytes(&bytes).is_err());
    }

    #[test]
    fn too_small_is_error() {
        let bytes = vec![0u8; PREAMBLE_SIZE - 1];
        assert!(FuncSts::from_bytes(&bytes).is_err());
    }

    #[test]
    fn zero_scan_record_size_is_error() {
        let mut bytes = make_preamble(32 + 48, 0, 1);
        bytes.extend(make_descriptor(62, 3, 0, "Collision Energy"));
        assert!(FuncSts::from_bytes(&bytes).is_err());
    }

    // -- Corpus integration tests --

    /// The shared vendor corpus lives in the SpecLance umbrella repo, checked
    /// out as a sibling of this repo; tests skip silently when it's absent
    /// (e.g. in a checkout that doesn't have SpecLance cloned alongside).
    fn corpus_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../SpecLance/corpus/waters")
    }

    #[test]
    fn corpus_pxd058812_collision_energy() {
        let raw = corpus_dir().join("PXD058812/molecular_mass_P15_01.raw/_FUNC001.STS");
        if !raw.exists() {
            return;
        }
        // Per docs/docs/format/07-func-sts.md: n_desc=25, scan_sz=63, n_scans=197,
        // Collision Energy = 10.0 eV throughout (non-IMS QTOF, MS-only function).
        let sts = FuncSts::from_path(&raw).unwrap();
        assert_eq!(sts.scan_count(), 197);
        let ce = sts.channel("Collision Energy").expect("channel missing");
        assert_eq!(ce.encoding, ChannelEncoding::F32);
        let v = sts.value_at(ce, 0).expect("value missing");
        assert!(
            (v - 10.0).abs() < 1e-3,
            "Collision Energy = {v}, expected 10.0"
        );
    }

    #[test]
    fn corpus_pxd075602_collision_energy() {
        let raw = corpus_dir().join("PXD075602/DHPR_11257-1.raw/_FUNC001.STS");
        if !raw.exists() {
            return;
        }
        // Per docs/docs/format/07-func-sts.md: n_desc=56, scan_sz=167, n_scans=1150,
        // Collision Energy = 4.0 eV.
        let sts = FuncSts::from_path(&raw).unwrap();
        assert_eq!(sts.scan_count(), 1150);
        let v = sts.collision_energy(0).expect("value missing");
        assert!(
            (v - 4.0).abs() < 1e-3,
            "Collision Energy = {v}, expected 4.0"
        );
    }
}
