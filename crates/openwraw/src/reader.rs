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
use crate::raw::functions_inf::{FunctionInfo, FunctionTable};
use crate::raw::header::{FunctionCal, Header};
use crate::raw::index::ScanIndex;

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

            functions.push(FunctionEntry {
                index: info.index,
                info: info.clone(),
                scan_index,
                dat_path,
                dat_size,
                encoding,
                cal,
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
        Ok(DecodedScan {
            function_index,
            scan_idx,
            retention_time_min: rt_min,
            spectrum: decoded,
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
fn scan_slice(entry: &FunctionEntry, scan_idx: usize) -> crate::Result<(u64, u64, f32)> {
    match &entry.scan_index {
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
            Ok((offset, length, rec.retention_time_min))
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
            Ok((offset, length, rec.retention_time_min))
        }
    }
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
