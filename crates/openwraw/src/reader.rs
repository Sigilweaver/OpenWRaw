//! High-level reader for a Waters `.raw/` bundle directory.
//!
//! Wraps the low-level primitives in [`crate::raw`] into a single
//! `Reader::open(dir)` entry point that:
//!
//! * Parses `_HEADER.TXT`, `_FUNCTNS.INF`, `_extern.inf`.
//! * Discovers every `_FUNCnnn.IDX` / `_FUNCnnn.DAT` pair on disk.
//! * Picks an encoding (A / B / C) per function based on the IDX stride
//!   (Variant A -> Encoding A) plus the instrument name on Variant B
//!   (`SYNAPT*` -> Encoding B IMS, anything else -> Encoding C).
//! * Provides [`Reader::iter_spectra`] which yields one decoded spectrum
//!   per scan, in `(function_index, scan_index_in_function)` order,
//!   skipping lock-mass functions.
//!
//! Mass-spec-core integration lives in [`crate::mzml`].

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::raw::data::{
    decode_encoding_a, decode_encoding_b, decode_encoding_c, DecodeParams, ImsSpectrum, Spectrum,
};
use crate::raw::extern_inf::ExternInf;
use crate::raw::func_sts::FuncSts;
use crate::raw::functions_inf::{FunctionInfo, FunctionTable};
use crate::raw::header::{FunctionCal, Header};
use crate::raw::index::ScanIndex;

/// Sanity-check a Variant A scan's declared centroid `peak_count` (`_FUNCnnn.IDX`
/// +0x10) against the number of peaks the decoder actually emitted.
///
/// `peak_count` is a MassLynx-computed centroid count, while
/// [`decode_encoding_a`] instead emits every non-sentinel, non-zero-intensity
/// 6-byte record - profile-mode oversampling means several raw records can
/// fold into a single centroid, so a centroid count can never exceed the
/// decoded record count (`docs/format/03-func-idx.md`'s Field +0x10 note
/// documents a corpus scan with 3,253 decoded records and a `peak_count` of
/// 47). Equality does not hold and isn't asserted; this only catches a
/// decode that produced implausibly *few* points for the peak count the
/// index claims.
fn check_peak_count_sanity(
    function_index: u32,
    scan_idx: usize,
    peak_count: u16,
    decoded_len: usize,
) {
    debug_assert!(
        peak_count as usize <= decoded_len,
        "function {function_index} scan {scan_idx}: _FUNCnnn.IDX peak_count \
         {peak_count} exceeds decoded peak count {decoded_len}"
    );
}

/// Which decoder applies to a given function's `_FUNCnnn.DAT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// 6-byte records, sentinel-anchored. Variant A index.
    A,
    /// 8-byte IMS records (count, dt_bin, tof_bin). Variant B index.
    B,
    /// 8-byte non-IMS records (intensity, sub_bin, tof_bin). Variant B index.
    C,
}

/// One acquisition function's static metadata, ready for decoding.
#[derive(Debug, Clone)]
pub struct FunctionEntry {
    /// 1-based function index.
    pub index: u32,
    /// `_FUNCTNS.INF` record for this function.
    pub info: FunctionInfo,
    /// Scan index parsed from `_FUNCnnn.IDX`.
    pub scan_index: ScanIndex,
    /// Path to `_FUNCnnn.DAT`.
    pub dat_path: PathBuf,
    /// Length of `_FUNCnnn.DAT` in bytes; used to size the trailing scan.
    pub dat_size: u64,
    /// Decoder this function's DAT requires.
    pub encoding: Encoding,
    /// Calibration polynomial pulled from `_HEADER.TXT`.
    pub cal: FunctionCal,
    /// Parsed `_FUNCnnn.STS` scan-statistics table, when the file is present
    /// and well-formed. `None` for bundles that lack it (older instrument
    /// generations) or where it fails to parse - a missing/bad STS file is
    /// not fatal to opening the bundle, since it only supplies supplementary
    /// per-scan housekeeping values (e.g. collision energy), not the peak
    /// data itself.
    pub sts: Option<FuncSts>,
}

impl FunctionEntry {
    /// Number of scans in this function.
    pub fn scan_count(&self) -> usize {
        self.scan_index.len()
    }

    /// Build the [`DecodeParams`] needed for one of the `decode_encoding_*`
    /// primitives.
    fn decode_params(&self, extern_inf: &ExternInf) -> DecodeParams {
        DecodeParams {
            a_us: extern_inf.a_us(),
            cal: self.cal.clone(),
            mz_low: self.info.mz_low as f64,
            mz_high: self.info.mz_high as f64,
            scan_time_ms: self.info.scan_time_s as f64 * 1000.0,
        }
    }
}

/// A fully-parsed Waters `.raw/` bundle, ready to stream spectra.
#[derive(Debug, Clone)]
pub struct Reader {
    pub dir: PathBuf,
    pub bundle_name: String,
    pub header: Header,
    pub extern_inf: ExternInf,
    pub functions: Vec<FunctionEntry>,
}

impl Reader {
    /// Open a `.raw/` bundle directory and parse every required side file.
    pub fn open<P: AsRef<Path>>(dir: P) -> crate::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let header = Header::from_path(&dir.join("_HEADER.TXT"))?;
        let extern_inf = ExternInf::from_path(&dir.join("_extern.inf"))?;
        let func_table = FunctionTable::from_path(&dir.join("_FUNCTNS.INF"))?;

        let instrument = header.instrument.clone().unwrap_or_default();
        let is_synapt = instrument.to_ascii_uppercase().starts_with("SYNAPT");

        let mut functions: Vec<FunctionEntry> = Vec::new();
        for info in &func_table.functions {
            let idx_name = format!("_FUNC{:03}.IDX", info.index);
            let dat_name = format!("_FUNC{:03}.DAT", info.index);
            let idx_path = dir.join(&idx_name);
            let dat_path = dir.join(&dat_name);
            if !idx_path.exists() || !dat_path.exists() {
                continue;
            }
            let scan_index = ScanIndex::from_path(&idx_path)?;
            let dat_size = fs::metadata(&dat_path)?.len();
            let encoding = match &scan_index {
                ScanIndex::A(_) => Encoding::A,
                ScanIndex::B(_) => {
                    if is_synapt {
                        Encoding::B
                    } else {
                        Encoding::C
                    }
                }
            };
            let cal = header
                .cal_functions
                .get(&info.index)
                .cloned()
                .unwrap_or_default();

            let sts_path = dir.join(format!("_FUNC{:03}.STS", info.index));
            let sts = FuncSts::from_path(&sts_path).ok();

            functions.push(FunctionEntry {
                index: info.index,
                info: info.clone(),
                scan_index,
                dat_path,
                dat_size,
                encoding,
                cal,
                sts,
            });
        }

        let bundle_name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "bundle.raw".into());

        Ok(Reader {
            dir,
            bundle_name,
            header,
            extern_inf,
            functions,
        })
    }

    /// Returns the total number of scans across all non-lock-mass functions.
    pub fn total_scan_count(&self) -> usize {
        self.functions
            .iter()
            .filter(|f| !f.info.is_lock_mass())
            .map(|f| f.scan_count())
            .sum()
    }

    /// Decode the `i`-th scan (0-based) of the given function.
    pub fn decode_scan(&self, function_index: u32, scan_idx: usize) -> crate::Result<DecodedScan> {
        let entry = self
            .functions
            .iter()
            .find(|f| f.index == function_index)
            .ok_or_else(|| {
                crate::Error::Parse(format!("function {function_index} not present in bundle"))
            })?;
        let (offset, length, rt_min) = scan_slice(entry, scan_idx)?;
        let bytes = read_slice(&entry.dat_path, offset, length)?;
        let params = entry.decode_params(&self.extern_inf);
        let decoded = match entry.encoding {
            Encoding::A => DecodedSpectrum::Plain(decode_encoding_a(&bytes, &params)?),
            Encoding::B => DecodedSpectrum::Ims(decode_encoding_b(&bytes, &params)?),
            Encoding::C => DecodedSpectrum::Plain(decode_encoding_c(&bytes, &params)?),
        };
        if let (ScanIndex::A(records), DecodedSpectrum::Plain(spectrum)) =
            (&entry.scan_index, &decoded)
        {
            if let Some(rec) = records.get(scan_idx) {
                check_peak_count_sanity(
                    function_index,
                    scan_idx,
                    rec.peak_count,
                    spectrum.mz.len(),
                );
            }
        }
        let collision_energy_ev = entry
            .sts
            .as_ref()
            .and_then(|sts| sts.collision_energy(scan_idx));
        let etd_fragmentation_mode = entry
            .sts
            .as_ref()
            .and_then(|sts| sts.etd_fragmentation_mode(scan_idx));
        Ok(DecodedScan {
            function_index,
            scan_idx,
            retention_time_min: rt_min,
            spectrum: decoded,
            collision_energy_ev,
            etd_fragmentation_mode,
        })
    }

    /// Iterate every non-lock-mass scan across the bundle, in function then
    /// scan order. Lock-mass / reference functions are skipped.
    pub fn iter_spectra(&self) -> impl Iterator<Item = crate::Result<DecodedScan>> + '_ {
        let plan: Vec<(u32, usize)> = self
            .functions
            .iter()
            .filter(|f| !f.info.is_lock_mass())
            .flat_map(|f| (0..f.scan_count()).map(move |i| (f.index, i)))
            .collect();
        plan.into_iter()
            .map(move |(fi, si)| self.decode_scan(fi, si))
    }
}

/// One scan after decoding.
#[derive(Debug, Clone)]
pub struct DecodedScan {
    pub function_index: u32,
    /// 0-based position within the function.
    pub scan_idx: usize,
    pub retention_time_min: f32,
    pub spectrum: DecodedSpectrum,
    /// Per-scan collision energy (eV) from `_FUNCnnn.STS`'s "Collision
    /// Energy" channel, when the file is present and defines that channel.
    pub collision_energy_ev: Option<f64>,
    /// Per-scan ETD Fragmentation Mode from `_FUNCnnn.STS`'s "ETD
    /// Fragmentation Mode" channel (seq 121): `0` for CID, non-zero for
    /// ETD, `None` when the file is absent or doesn't define the channel.
    pub etd_fragmentation_mode: Option<f64>,
}

/// Decoded payload of a scan; varies by encoding.
#[derive(Debug, Clone)]
pub enum DecodedSpectrum {
    /// Output of Encoding A or C.
    Plain(Spectrum),
    /// Output of Encoding B (IMS).
    Ims(ImsSpectrum),
}

/// Resolve the byte slice for scan `scan_idx` within `entry`'s DAT file.
///
/// Returns `(offset, length, retention_time_min)`. Trailing scans take the
/// length implied by `entry.dat_size`.
///
/// `dat_offset` and (for Variant B) the next record's `dat_offset` are raw
/// fields read straight from the `.IDX` file, so `length` is capped against
/// `entry.dat_size` (the real, already-known size of the paired `.DAT` file)
/// before returning: an IDX record claiming a scan larger than the DAT file
/// that actually exists must not be able to force an allocation sized from
/// unvalidated file-controlled offsets in `read_slice`.
fn scan_slice(entry: &FunctionEntry, scan_idx: usize) -> crate::Result<(u64, u64, f32)> {
    let (offset, length, retention_time_min) = match &entry.scan_index {
        ScanIndex::A(records) => {
            let rec = records.get(scan_idx).ok_or_else(|| {
                crate::Error::Parse(format!(
                    "function {} scan {} out of range",
                    entry.index, scan_idx
                ))
            })?;
            // Variant A stores n_records directly: each record is 6 bytes.
            let offset = rec.dat_offset as u64;
            let length = (rec.n_records as u64) * 6;
            (offset, length, rec.retention_time_min)
        }
        ScanIndex::B(records) => {
            let rec = records.get(scan_idx).ok_or_else(|| {
                crate::Error::Parse(format!(
                    "function {} scan {} out of range",
                    entry.index, scan_idx
                ))
            })?;
            let offset = rec.dat_offset as u64;
            let next_offset = records
                .get(scan_idx + 1)
                .map(|r| r.dat_offset as u64)
                .unwrap_or(entry.dat_size);
            let length = next_offset.saturating_sub(offset);
            (offset, length, rec.retention_time_min)
        }
    };
    let remaining = entry.dat_size.saturating_sub(offset);
    Ok((offset, length.min(remaining), retention_time_min))
}

fn read_slice(path: &Path, offset: u64, length: u64) -> crate::Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = fs::File::open(path)?;
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; length as usize];
    f.read_exact(&mut buf)?;
    Ok(buf)
}

/// Group functions by encoding for quick reporting.
pub fn encoding_counts(reader: &Reader) -> BTreeMap<&'static str, usize> {
    let mut out = BTreeMap::new();
    for f in &reader.functions {
        let key = match f.encoding {
            Encoding::A => "A",
            Encoding::B => "B",
            Encoding::C => "C",
        };
        *out.entry(key).or_insert(0) += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::functions_inf::FunctionInfo;
    use crate::raw::header::FunctionCal;
    use crate::raw::index::{ScanIndexA, ScanIndexB};

    fn dummy_info(index: u32) -> FunctionInfo {
        FunctionInfo {
            index,
            function_type: 0,
            scan_subtype: 0,
            cycle_time_s: 0.0,
            interscan_delay_s: 0.0,
            scan_time_s: 0.0,
            tof_depth: 0,
            mz_low: 0.0,
            mz_high: 0.0,
        }
    }

    fn entry_with(scan_index: ScanIndex, dat_size: u64) -> FunctionEntry {
        FunctionEntry {
            index: 1,
            info: dummy_info(1),
            scan_index,
            dat_path: PathBuf::new(),
            dat_size,
            encoding: Encoding::C,
            cal: FunctionCal::default(),
            sts: None,
        }
    }

    // A corrupt/malicious IDX can claim a scan far larger than the real DAT
    // file: dat_offset=0 for this scan, dat_offset=u32::MAX-1 for the "next"
    // scan used to compute Variant B's length by subtraction. Before this
    // was capped, `read_slice` would allocate a `Vec` sized from that
    // difference (up to ~4.29 GB) regardless of how small the real DAT file
    // on disk actually is - which aborts the process under a virtual-memory
    // limit rather than returning a recoverable error.
    #[test]
    fn variant_b_scan_slice_caps_length_to_dat_size() {
        let entry = entry_with(
            ScanIndex::B(vec![
                ScanIndexB {
                    dat_offset: 0,
                    retention_time_min: 0.0,
                },
                ScanIndexB {
                    dat_offset: u32::MAX - 1,
                    retention_time_min: 0.1,
                },
            ]),
            64, // real DAT file is tiny
        );
        let (offset, length, _) = scan_slice(&entry, 0).unwrap();
        assert_eq!(offset, 0);
        assert!(length <= 64, "length {length} exceeds dat_size 64");
    }

    #[test]
    fn variant_b_offset_beyond_dat_size_yields_zero_length() {
        let entry = entry_with(
            ScanIndex::B(vec![ScanIndexB {
                dat_offset: 1_000_000,
                retention_time_min: 0.0,
            }]),
            64,
        );
        let (_, length, _) = scan_slice(&entry, 0).unwrap();
        assert_eq!(length, 0);
    }

    #[test]
    fn variant_b_normal_scan_is_unaffected() {
        let entry = entry_with(
            ScanIndex::B(vec![
                ScanIndexB {
                    dat_offset: 0,
                    retention_time_min: 0.0,
                },
                ScanIndexB {
                    dat_offset: 40,
                    retention_time_min: 0.1,
                },
            ]),
            100,
        );
        let (offset, length, _) = scan_slice(&entry, 0).unwrap();
        assert_eq!(offset, 0);
        assert_eq!(length, 40);
    }

    #[test]
    fn variant_a_scan_slice_caps_length_to_dat_size() {
        let entry = entry_with(
            ScanIndex::A(vec![ScanIndexA {
                dat_offset: 0,
                n_records: u16::MAX, // claims 393,210 bytes
                retention_time_min: 0.0,
                peak_count: 0,
            }]),
            64,
        );
        let (_, length, _) = scan_slice(&entry, 0).unwrap();
        assert!(length <= 64, "length {length} exceeds dat_size 64");
    }

    #[test]
    fn variant_a_normal_scan_is_unaffected() {
        let entry = entry_with(
            ScanIndex::A(vec![ScanIndexA {
                dat_offset: 0,
                n_records: 5,
                retention_time_min: 0.0,
                peak_count: 0,
            }]),
            100,
        );
        let (_, length, _) = scan_slice(&entry, 0).unwrap();
        assert_eq!(length, 30);
    }

    // -- check_peak_count_sanity --

    #[test]
    fn peak_count_sanity_passes_at_the_documented_corpus_ratio() {
        // docs/format/03-func-idx.md: PXD058812 scan with 3,253 decoded
        // records and a `peak_count` of 47.
        check_peak_count_sanity(1, 0, 47, 3253);
    }

    #[test]
    fn peak_count_sanity_passes_when_equal() {
        check_peak_count_sanity(1, 0, 10, 10);
    }

    #[test]
    fn peak_count_sanity_passes_for_blank_scan() {
        check_peak_count_sanity(1, 0, 0, 0);
    }

    #[test]
    #[should_panic(expected = "exceeds decoded peak count")]
    fn peak_count_sanity_panics_when_peak_count_exceeds_decoded_len() {
        check_peak_count_sanity(1, 0, 100, 5);
    }
}
