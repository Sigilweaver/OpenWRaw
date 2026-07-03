// Parser for _CHROMS.INF and _CHROnnnn.DAT files.
//
// _CHROMS.INF describes each recorded instrument channel (pump pressure,
// flow rate, temperature, etc.) with source type, name, scale factor and units.
// _CHROnnnn.DAT holds the corresponding (RT, value) time-series for that channel.

use std::path::Path;

const HEADER_SIZE: usize = 128;
const RECORD_SIZE: usize = 85;
/// Number of meta records that always precede the data records.
const N_META: usize = 2;

/// CHRO file header size (preamble + 2 descriptor records).
const CHRO_DATA_OFFSET: usize = 128;

/// Description of a single recorded chromatographic channel from `_CHROMS.INF`.
#[derive(Debug, Clone)]
pub struct ChromChannel {
    /// 0-based index among data records only (meta records excluded).
    pub index: usize,
    /// Source device type: 4 = BSM pump, 1 = column/sample device.
    pub source_type: u32,
    /// Channel name decoded from Windows-1252 (e.g. "BSM Composition B").
    pub name: String,
    /// Scale factor from the `$CC$` spec string (e.g. `1.0` or `0.1`).
    pub scale_f: f64,
    /// Engineering units decoded from the `$CC$` spec string (e.g. "%", "C").
    pub units: String,
}

/// Parsed contents of a `_CHROMS.INF` file.
#[derive(Debug, Clone)]
pub struct ChromsInf {
    pub channels: Vec<ChromChannel>,
}

impl ChromsInf {
    /// Read and parse a `_CHROMS.INF` file at the given path.
    pub fn from_path(path: &Path) -> crate::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Parse from an in-memory byte slice (useful for testing).
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
        let min_size = HEADER_SIZE + N_META * RECORD_SIZE;
        if bytes.len() < min_size {
            return Err(crate::Error::Parse(format!(
                "_CHROMS.INF too small: {} bytes (need at least {})",
                bytes.len(),
                min_size
            )));
        }

        let rec_size = crate::bytes::read_u16_le(bytes, 4)? as usize;
        if rec_size != RECORD_SIZE {
            return Err(crate::Error::Parse(format!(
                "_CHROMS.INF: record size field is {rec_size}, expected {RECORD_SIZE}"
            )));
        }

        let n_meta = crate::bytes::read_u16_le(bytes, 6)? as usize;
        let data_start = HEADER_SIZE + n_meta * RECORD_SIZE;
        if bytes.len() < data_start {
            return Err(crate::Error::Parse(format!(
                "_CHROMS.INF: file too small for declared {n_meta} meta records"
            )));
        }

        let remaining = bytes.len() - data_start;
        if remaining % RECORD_SIZE != 0 {
            return Err(crate::Error::Parse(format!(
                "_CHROMS.INF: data section size {remaining} is not a multiple of {RECORD_SIZE}"
            )));
        }

        let n_data = remaining / RECORD_SIZE;
        let mut channels = Vec::with_capacity(n_data);

        for i in 0..n_data {
            let off = data_start + i * RECORD_SIZE;
            let rec = &bytes[off..off + RECORD_SIZE];

            let source_type = crate::bytes::read_u32_le(rec, 0)?;

            // Bytes 4..85: null-padded channel name followed by null-padded $CC$ string.
            let payload = &rec[4..RECORD_SIZE];
            let name_end = payload
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(payload.len());
            let name = decode_cp1252(&payload[..name_end]);

            let (scale_f, units) = payload
                .windows(4)
                .position(|w| w == b"$CC$")
                .and_then(|off| parse_cc_spec(&payload[off..]))
                .unwrap_or((1.0, String::new()));

            channels.push(ChromChannel {
                index: i,
                source_type,
                name,
                scale_f,
                units,
            });
        }

        Ok(ChromsInf { channels })
    }

    /// Returns the 1-based CHRO file number for a given data record index (0-based).
    ///
    /// CHRO files cover ALL records in `_CHROMS.INF` (meta + data) in order, numbered
    /// from 1. The first two slots are always the meta records, so data record 0 maps
    /// to `_CHRO0003.DAT` (index 3).
    pub fn chro_number_for_channel(&self, channel_index: usize) -> usize {
        N_META + channel_index + 1
    }
}

/// A single (retention-time, value) sample from a `_CHROnnnn.DAT` file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromPoint {
    /// Retention time in minutes.
    pub rt_min: f32,
    /// Instrument channel value in the units given by `ChromChannel::units`.
    pub value: f32,
}

/// Read all (RT, value) samples from a `_CHROnnnn.DAT` file.
pub fn read_chro_dat(path: &Path) -> crate::Result<Vec<ChromPoint>> {
    let bytes = std::fs::read(path)?;
    parse_chro_bytes(&bytes)
}

/// Parse from an in-memory byte slice (useful for testing).
pub fn parse_chro_bytes(bytes: &[u8]) -> crate::Result<Vec<ChromPoint>> {
    if bytes.len() < CHRO_DATA_OFFSET {
        return Err(crate::Error::Parse(format!(
            "_CHRO*.DAT too small: {} bytes (need at least {})",
            bytes.len(),
            CHRO_DATA_OFFSET
        )));
    }

    let data = &bytes[CHRO_DATA_OFFSET..];
    if data.len() % 8 != 0 {
        return Err(crate::Error::Parse(format!(
            "_CHRO*.DAT data section size {} is not a multiple of 8",
            data.len()
        )));
    }

    let n = data.len() / 8;
    let mut points = Vec::with_capacity(n);
    for i in 0..n {
        let rt_min = crate::bytes::read_f32_le(data, i * 8)?;
        let value = crate::bytes::read_f32_le(data, i * 8 + 4)?;
        points.push(ChromPoint { rt_min, value });
    }
    Ok(points)
}

// -- Helpers --

/// Decode bytes as Windows-1252 (the encoding Waters uses for channel names).
///
/// For bytes 0x00-0x7F: identical to ASCII.
/// For bytes 0x80-0xFF: mapped via Windows-1252 to Unicode.
fn decode_cp1252(bytes: &[u8]) -> String {
    // Windows-1252 supplement table for bytes 0x80-0x9F.
    // Bytes 0xA0-0xFF map directly to U+00A0-U+00FF (same as Latin-1).
    const W1252: [char; 32] = [
        '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}',
        '\u{017D}', '\u{008F}', '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
        '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
    ];
    bytes
        .iter()
        .map(|&b| match b {
            0x00..=0x7F => b as char,
            0x80..=0x9F => W1252[(b - 0x80) as usize],
            _ => char::from_u32(b as u32).unwrap_or('\u{FFFD}'),
        })
        .collect()
}

/// Parse a `$CC$` spec string starting at `bytes`.
///
/// Format: `$CC$,<scale_f>,<type_code>,<lo>,<hi>,<units>` (null-terminated).
/// Returns `(scale_f, units)` on success.
fn parse_cc_spec(bytes: &[u8]) -> Option<(f64, String)> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let raw = &bytes[..end];
    // Split on ASCII comma without converting to UTF-8 first (units may be Windows-1252).
    let parts: Vec<&[u8]> = raw.splitn(6, |&b| b == b',').collect();
    if parts.len() < 6 {
        return None;
    }
    let scale_f = std::str::from_utf8(parts[1])
        .ok()?
        .trim()
        .parse::<f64>()
        .ok()?;
    let units = decode_cp1252(parts[5]);
    Some((scale_f, units))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Helpers --

    fn make_header(n_meta: u16, n_data: usize) -> Vec<u8> {
        let mut h = vec![0u8; HEADER_SIZE];
        h[0..2].copy_from_slice(&128u16.to_le_bytes()); // header_size
        h[2..4].copy_from_slice(&1u16.to_le_bytes()); // version
        h[4..6].copy_from_slice(&(RECORD_SIZE as u16).to_le_bytes()); // record_size
        h[6..8].copy_from_slice(&n_meta.to_le_bytes()); // n_meta
        let _ = n_data; // used by caller to set file size
        h
    }

    fn make_meta_record(meta_type: u32, name: &str) -> Vec<u8> {
        let mut r = vec![0u8; RECORD_SIZE];
        r[0..4].copy_from_slice(&meta_type.to_le_bytes());
        let n = name.len().min(80);
        r[4..4 + n].copy_from_slice(&name.as_bytes()[..n]);
        r
    }

    fn make_data_record(source_type: u32, name: &str, cc_spec: &str) -> Vec<u8> {
        let mut r = vec![0u8; RECORD_SIZE];
        r[0..4].copy_from_slice(&source_type.to_le_bytes());
        let payload = &mut r[4..RECORD_SIZE];
        let name_bytes = name.as_bytes();
        let n = name_bytes.len().min(payload.len() - 1);
        payload[..n].copy_from_slice(&name_bytes[..n]);
        // null byte at n (already zero)
        let cc_bytes = cc_spec.as_bytes();
        let cc_start = n + 1;
        let cc_len = cc_bytes.len().min(payload.len() - cc_start - 1);
        payload[cc_start..cc_start + cc_len].copy_from_slice(&cc_bytes[..cc_len]);
        r
    }

    fn make_chroms_inf(n_data: usize) -> Vec<u8> {
        let mut bytes = make_header(N_META as u16, n_data);
        bytes.extend(make_meta_record(1, "Flags"));
        bytes.extend(make_meta_record(2, "Description"));
        for i in 0..n_data {
            let cc = format!("$CC$,1.0,3,0,0,{}", if i == 0 { "psi" } else { "%" });
            bytes.extend(make_data_record(4, &format!("Channel {i}"), &cc));
        }
        bytes
    }

    // -- _CHROMS.INF tests --

    #[test]
    fn parse_empty_channels() {
        let bytes = make_chroms_inf(0);
        let ci = ChromsInf::from_bytes(&bytes).unwrap();
        assert!(ci.channels.is_empty());
    }

    #[test]
    fn parse_single_channel() {
        let bytes = make_chroms_inf(1);
        let ci = ChromsInf::from_bytes(&bytes).unwrap();
        assert_eq!(ci.channels.len(), 1);
        assert_eq!(ci.channels[0].source_type, 4);
        assert_eq!(ci.channels[0].name, "Channel 0");
        assert!((ci.channels[0].scale_f - 1.0).abs() < 1e-9);
        assert_eq!(ci.channels[0].units, "psi");
    }

    #[test]
    fn parse_multiple_channels() {
        let bytes = make_chroms_inf(3);
        let ci = ChromsInf::from_bytes(&bytes).unwrap();
        assert_eq!(ci.channels.len(), 3);
        for (i, ch) in ci.channels.iter().enumerate() {
            assert_eq!(ch.index, i);
        }
    }

    #[test]
    fn chro_number_offset_is_meta_plus_one() {
        let bytes = make_chroms_inf(5);
        let ci = ChromsInf::from_bytes(&bytes).unwrap();
        // channel 0 → CHRO file 3 (meta 0, meta 1, then data records)
        assert_eq!(ci.chro_number_for_channel(0), 3);
        assert_eq!(ci.chro_number_for_channel(4), 7);
    }

    #[test]
    fn too_small_is_error() {
        let bytes = vec![0u8; HEADER_SIZE - 1];
        assert!(ChromsInf::from_bytes(&bytes).is_err());
    }

    #[test]
    fn wrong_record_size_is_error() {
        let mut bytes = make_chroms_inf(1);
        // Corrupt the record_size field
        bytes[4..6].copy_from_slice(&99u16.to_le_bytes());
        assert!(ChromsInf::from_bytes(&bytes).is_err());
    }

    #[test]
    fn windows1252_units_decoded_correctly() {
        // Build a record with µ (0xB5) and ° (0xB0) in the units.
        let mut r = vec![0u8; RECORD_SIZE];
        r[0..4].copy_from_slice(&4u32.to_le_bytes());
        let payload = &mut r[4..RECORD_SIZE];
        let name = b"Flow";
        payload[..name.len()].copy_from_slice(name);
        // cc: $CC$,1.0,3,0,0,µL/min  (µ = 0xB5)
        let cc: Vec<u8> = b"$CC$,1.0,3,0,0,\xB5L/min".to_vec();
        let cc_start = name.len() + 1;
        payload[cc_start..cc_start + cc.len()].copy_from_slice(&cc);

        let mut bytes = make_header(N_META as u16, 0);
        bytes.extend(make_meta_record(1, "Flags"));
        bytes.extend(make_meta_record(2, "Description"));
        bytes.extend(r);
        let ci = ChromsInf::from_bytes(&bytes).unwrap();
        assert_eq!(ci.channels[0].units, "\u{00B5}L/min"); // µL/min
    }

    // -- _CHROnnnn.DAT tests --

    fn make_chro_dat(points: &[(f32, f32)]) -> Vec<u8> {
        let mut bytes = vec![0u8; CHRO_DATA_OFFSET];
        // Write minimal valid preamble
        bytes[0..2].copy_from_slice(&128u16.to_le_bytes()); // data_offset
        bytes[2..4].copy_from_slice(&1u16.to_le_bytes()); // version
        bytes[4..6].copy_from_slice(&8u16.to_le_bytes()); // bytes_per_record
        bytes[6..8].copy_from_slice(&2u16.to_le_bytes()); // n_descriptor_records
        for &(rt, val) in points {
            bytes.extend_from_slice(&rt.to_le_bytes());
            bytes.extend_from_slice(&val.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn chro_dat_empty_points() {
        let bytes = make_chro_dat(&[]);
        let pts = parse_chro_bytes(&bytes).unwrap();
        assert!(pts.is_empty());
    }

    #[test]
    fn chro_dat_single_point() {
        let bytes = make_chro_dat(&[(1.23_f32, 456.78_f32)]);
        let pts = parse_chro_bytes(&bytes).unwrap();
        assert_eq!(pts.len(), 1);
        assert!((pts[0].rt_min - 1.23).abs() < 1e-5, "rt={}", pts[0].rt_min);
        assert!(
            (pts[0].value - 456.78).abs() < 0.01,
            "value={}",
            pts[0].value
        );
    }

    #[test]
    fn chro_dat_multiple_points_sorted_by_rt() {
        let expected = vec![(0.0_f32, 100.0_f32), (0.5, 200.0), (1.0, 150.0)];
        let bytes = make_chro_dat(&expected);
        let pts = parse_chro_bytes(&bytes).unwrap();
        assert_eq!(pts.len(), 3);
        for (i, &(rt, val)) in expected.iter().enumerate() {
            assert!((pts[i].rt_min - rt).abs() < 1e-6);
            assert!((pts[i].value - val).abs() < 0.01);
        }
    }

    #[test]
    fn chro_dat_too_small_is_error() {
        let bytes = vec![0u8; CHRO_DATA_OFFSET - 1];
        assert!(parse_chro_bytes(&bytes).is_err());
    }

    #[test]
    fn chro_dat_odd_data_size_is_error() {
        let mut bytes = make_chro_dat(&[(1.0, 2.0)]);
        bytes.push(0); // 1 extra byte after 8-byte record → 9 bytes data, not multiple of 8
        assert!(parse_chro_bytes(&bytes).is_err());
    }

    // -- Corpus integration tests --

    #[test]
    fn corpus_ctpa_chroms_inf() {
        use std::path::Path;
        let raw = Path::new("/workspaces/OpenWRaw/corpus/PXD068881/20220517_CtpA_1076_2h_1.raw");
        if !raw.exists() {
            return;
        }
        // PXD068881: 5 data channels, file = 723 bytes
        let ci = ChromsInf::from_path(&raw.join("_CHROMS.INF")).unwrap();
        assert_eq!(ci.channels.len(), 5, "CtpA should have 5 data channels");
        // channel 0: BSM Composition B, source_type=4
        assert_eq!(ci.channels[0].source_type, 4);
        assert!(
            ci.channels[0].name.contains("BSM"),
            "name={}",
            ci.channels[0].name
        );
        // All channels have a non-empty units string
        for ch in &ci.channels {
            assert!(!ch.units.is_empty(), "channel {} has empty units", ch.name);
        }
        // CHRO file numbering
        assert_eq!(ci.chro_number_for_channel(0), 3);
        assert_eq!(ci.chro_number_for_channel(4), 7);
    }

    #[test]
    fn corpus_ctpa_chro_dat() {
        use std::path::Path;
        let raw = Path::new("/workspaces/OpenWRaw/corpus/PXD068881/20220517_CtpA_1076_2h_1.raw");
        if !raw.exists() {
            return;
        }
        // _CHRO003.DAT = first data channel (BSM Composition B, channel index 0)
        let ci = ChromsInf::from_path(&raw.join("_CHROMS.INF")).unwrap();
        let chro_num = ci.chro_number_for_channel(0); // = 3
        let pts = read_chro_dat(&raw.join(format!("_CHRO{chro_num:03}.DAT"))).unwrap();
        assert!(!pts.is_empty(), "should have time-series data");
        // RT should be monotonically non-decreasing and within run duration.
        let mut prev = f32::NEG_INFINITY;
        for p in &pts {
            assert!(p.rt_min >= prev, "RT not monotone: {prev} -> {}", p.rt_min);
            assert!(p.rt_min <= 15.0, "RT {} out of expected range", p.rt_min);
            prev = p.rt_min;
        }
    }

    #[test]
    fn corpus_dhpr_chroms_inf() {
        use std::path::Path;
        let raw = Path::new("/workspaces/OpenWRaw/corpus/PXD075602/DHPR_11257-1.raw");
        if !raw.exists() {
            return;
        }
        // PXD075602: 3 data channels, file = 553 bytes
        let ci = ChromsInf::from_path(&raw.join("_CHROMS.INF")).unwrap();
        assert_eq!(ci.channels.len(), 3, "DHPR should have 3 data channels");
        assert_eq!(ci.chro_number_for_channel(0), 3);
    }
}
