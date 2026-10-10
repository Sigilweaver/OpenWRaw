//! High-level reader for a Waters `.raw/` bundle directory.
//!
//! Wraps the low-level primitives in [`crate::raw`] into a single
//! `Reader::open(dir)` entry point that:
//!
//! * Parses `_HEADER.TXT`, `_FUNCTNS.INF`, `_extern.inf`.
//! * Discovers every `_FUNCnnn.IDX` / `_FUNCnnn.DAT` pair on disk.
//! * Picks an encoding (A / D / E) per function from the DAT record width:
//!   the 22-byte index gives it through record counts and offsets, and for
//!   the 30-byte index it is read from the position words of sampled scans.
//! * Provides [`Reader::iter_spectra`] which yields one decoded spectrum
//!   per scan, in `(function_index, scan_index_in_function)` order,
//!   skipping lock-mass functions.
//!
//! Mass-spec-core integration lives in [`crate::mzml`].

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::raw::data::{
    decode_encoding_a, decode_encoding_d, decode_encoding_e, variant_b_record_width, DecodeParams,
    Spectrum,
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
    // `peak_count` comes from the file, so a mismatch is a data problem to
    // report, not an internal invariant to assert.
    if peak_count as usize > decoded_len {
        log::warn!(
            "function {function_index} scan {scan_idx}: _FUNCnnn.IDX peak_count \
             {peak_count} exceeds decoded peak count {decoded_len}"
        );
    }
}

/// Which decoder applies to a given function's `_FUNCnnn.DAT`.
///
/// The encoding names the DAT record layout. It is independent of the index
/// variant (`ScanIndex::A` or `ScanIndex::B`), except that Encoding A has
/// only been seen with the 22-byte index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Encoding {
    /// 6-byte records (u16 ion count, floating-point m/z). Variant A index.
    A,
    /// 8-byte records (16.16 intensity, floating-point m/z). Either index.
    D,
    /// 12-byte records (compressed intensity, floating-point m/z, auxiliary
    /// word). Either index.
    E,
}

impl Encoding {
    /// Whether this encoding stores centroids rather than profile points.
    ///
    /// Encodings A and D store profile data: peaks span runs of points one
    /// sample apart. Encoding E stores isolated, irregularly spaced points
    /// (one per peak) in the corpus functions that use it behind the 30-byte
    /// index; the LCT Premier functions behind the 22-byte index share its
    /// record layout and are treated the same, which the public corpus has
    /// not confirmed from their point spacing (`docs/format/04-func-dat.md`).
    pub fn is_centroided(self) -> bool {
        match self {
            Encoding::A | Encoding::D => false,
            Encoding::E => true,
        }
    }
}

/// One acquisition function's static metadata, ready for decoding.
#[derive(Debug, Clone)]
#[non_exhaustive]
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
    fn decode_params(&self) -> DecodeParams {
        DecodeParams::new(self.cal.clone())
    }
}

/// A fully-parsed Waters `.raw/` bundle, ready to stream spectra.
#[derive(Debug, Clone)]
#[non_exhaustive]
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
        log::debug!("opening {}", dir.display());
        let header_path = required_file(&dir, "_HEADER.TXT")?;
        let header = Header::from_path(&header_path)
            .map_err(|e| e.with_context(format!("reading {}", header_path.display())))?;
        let extern_path = required_file(&dir, "_extern.inf")?;
        let extern_inf = ExternInf::from_path(&extern_path)
            .map_err(|e| e.with_context(format!("reading {}", extern_path.display())))?;
        let functions_path = required_file(&dir, "_FUNCTNS.INF")?;
        let func_table = FunctionTable::from_path(&functions_path)
            .map_err(|e| e.with_context(format!("reading {}", functions_path.display())))?;

        let instrument = header.instrument.clone().unwrap_or_default();
        log::debug!(
            "instrument {instrument:?}; Lteff {} mm, Veff {} V, pusher interval {}; \
             {} functions in _FUNCTNS.INF; T1 calibration for functions {:?}",
            extern_inf.lteff_mm,
            extern_inf.veff_v,
            extern_inf
                .pusher_interval_us
                .map_or("absent".to_owned(), |us| format!("{us} us")),
            func_table.functions.len(),
            header.cal_functions.keys().collect::<Vec<_>>(),
        );

        let mut functions: Vec<FunctionEntry> = Vec::new();
        for info in &func_table.functions {
            let idx_name = format!("_FUNC{:03}.IDX", info.index);
            let dat_name = format!("_FUNC{:03}.DAT", info.index);
            let (Some(idx_path), Some(dat_path)) =
                (find_file(&dir, &idx_name)?, find_file(&dir, &dat_name)?)
            else {
                log::warn!(
                    "function {}: {idx_name} or {dat_name} missing in {}; skipping function",
                    info.index,
                    dir.display()
                );
                continue;
            };
            let scan_index = ScanIndex::from_path(&idx_path)
                .map_err(|e| e.with_context(format!("reading {}", idx_path.display())))?;
            let dat_size = fs::metadata(&dat_path)?.len();
            let (encoding, reason) = match &scan_index {
                ScanIndex::A(records) => match variant_a_record_width(records) {
                    Some(12) => (Encoding::E, "22-byte index, 12-byte records".to_owned()),
                    Some(8) => (Encoding::D, "22-byte index, 8-byte records".to_owned()),
                    Some(_) => (Encoding::A, "22-byte index, 6-byte records".to_owned()),
                    None => (
                        Encoding::A,
                        "22-byte index, record width not established from offsets; \
                         assuming 6-byte records"
                            .to_owned(),
                    ),
                },
                ScanIndex::B(records) => {
                    variant_b_encoding(info.index, &dat_path, records, dat_size)
                }
            };
            let cal = match header.cal_functions.get(&info.index) {
                Some(cal) => cal.clone(),
                None => {
                    log::warn!(
                        "function {}: no Cal Function line in _HEADER.TXT; m/z is uncalibrated",
                        info.index
                    );
                    FunctionCal::default()
                }
            };

            let sts = match find_file(&dir, &format!("_FUNC{:03}.STS", info.index))? {
                None => {
                    log::debug!("function {}: no STS file", info.index);
                    None
                }
                Some(path) => match FuncSts::from_path(&path) {
                    Ok(sts) => Some(sts),
                    Err(e) => {
                        log::warn!(
                            "function {}: ignoring unreadable {}: {e}",
                            info.index,
                            path.display()
                        );
                        None
                    }
                },
            };

            if let Some(end) = index_end(&scan_index, encoding) {
                if end > dat_size {
                    log::warn!(
                        "function {}: index addresses {end} bytes but {} is {dat_size} bytes",
                        info.index,
                        dat_path.display()
                    );
                }
            }
            log::debug!(
                "function {}: {} scans, m/z {}-{}, subtype {:#04x}{}, encoding {encoding:?} \
                 ({reason}), calibration {:?} with {} coefficients, DAT {dat_size} bytes",
                info.index,
                scan_index.len(),
                info.mz_low,
                info.mz_high,
                info.scan_subtype,
                if info.is_lock_mass() {
                    " (lock mass)"
                } else {
                    ""
                },
                cal.cal_type,
                cal.coeffs.len(),
            );

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
        log::debug!(
            "opened {bundle_name}: {} of {} functions readable",
            functions.len(),
            func_table.functions.len()
        );

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
        self.decode_scan_inner(function_index, scan_idx)
            .map_err(|e| {
                e.with_context(format!(
                    "{} function {function_index} scan {scan_idx}",
                    self.dir.display()
                ))
            })
    }

    fn decode_scan_inner(
        &self,
        function_index: u32,
        scan_idx: usize,
    ) -> crate::Result<DecodedScan> {
        let entry = self
            .functions
            .iter()
            .find(|f| f.index == function_index)
            .ok_or_else(|| {
                crate::Error::Parse(format!("function {function_index} not present in bundle"))
            })?;
        let (offset, length, rt_min) = scan_slice(entry, scan_idx)?;
        log::trace!(
            "function {function_index} scan {scan_idx}: {:?} bytes {offset}..{} of {}, RT {rt_min} min",
            entry.encoding,
            offset + length,
            entry.dat_path.display()
        );
        let bytes = read_slice(&entry.dat_path, offset, length)?;
        let params = entry.decode_params();
        let spectrum = match entry.encoding {
            Encoding::A => decode_encoding_a(&bytes, &params)?,
            Encoding::D => decode_encoding_d(&bytes, &params)?,
            Encoding::E => decode_encoding_e(&bytes, &params)?,
        };
        // Only Encoding A's 6-byte records are known to relate to the IDX
        // `peak_count` this way (docs/format/03-func-idx.md).
        if let (Encoding::A, ScanIndex::A(records)) = (entry.encoding, &entry.scan_index) {
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
            spectrum,
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

/// MassLynx exports may lowercase names or prefix every side file with a
/// sample identifier. Prefer an exact case-insensitive name before a suffix.
pub(crate) fn find_file(dir: &Path, name: &str) -> crate::Result<Option<PathBuf>> {
    let wanted = name.to_ascii_uppercase();
    let mut suffix = None;
    let entries = fs::read_dir(dir)
        .map_err(|e| crate::Error::from(e).with_context(format!("listing {}", dir.display())))?;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let actual = entry.file_name().to_string_lossy().to_ascii_uppercase();
        if actual == wanted {
            if entry.file_name().to_string_lossy() != name {
                log::trace!("{name}: using differently cased {}", entry.path().display());
            }
            return Ok(Some(entry.path()));
        }
        if actual.ends_with(&wanted) {
            if suffix.is_some() {
                return Err(crate::Error::Parse(format!(
                    "multiple files match {name} in {}",
                    dir.display()
                )));
            }
            suffix = Some(entry.path());
        }
    }
    if let Some(path) = &suffix {
        log::debug!("{name}: using prefixed {}", path.display());
    }
    Ok(suffix)
}

/// Byte offset one past the last scan the index addresses, when the index
/// records it directly (Variant A counts; Variant B's last scan runs to EOF).
fn index_end(scan_index: &ScanIndex, encoding: Encoding) -> Option<u64> {
    let ScanIndex::A(records) = scan_index else {
        return None;
    };
    let width = match encoding {
        Encoding::A => 6,
        Encoding::D => 8,
        Encoding::E => 12,
    };
    records
        .iter()
        .map(|r| u64::from(r.dat_offset) + u64::from(r.n_records) * width)
        .max()
}

fn required_file(dir: &Path, name: &str) -> crate::Result<PathBuf> {
    find_file(dir, name)?.ok_or_else(|| {
        crate::Error::Parse(format!("required file {name} missing in {}", dir.display()))
    })
}

/// Scans sampled, evenly spaced, when judging a Variant B record width.
const VARIANT_B_WIDTH_SAMPLES: usize = 16;

/// Choose between Encodings D and E for a function with a 30-byte index by
/// sampling scan slices from its DAT file. A failed read is logged and does
/// not fail `Reader::open`; scans that cannot be read report their own errors
/// when decoded.
fn variant_b_encoding(
    function: u32,
    dat_path: &Path,
    records: &[crate::raw::index::ScanIndexB],
    dat_size: u64,
) -> (Encoding, String) {
    match sample_variant_b_width(dat_path, records, dat_size) {
        Ok(Some((12, scan))) => (
            Encoding::E,
            format!("30-byte index, 12-byte records judged from scan {scan}"),
        ),
        Ok(Some((_, scan))) => (
            Encoding::D,
            format!("30-byte index, 8-byte records judged from scan {scan}"),
        ),
        Err(e) => {
            // Each scan that cannot be read reports its own error when
            // decoded; the other functions in the bundle stay readable.
            log::warn!(
                "function {}: could not sample scans of {}: {e}; \
                 assuming 8-byte records",
                function,
                dat_path.display()
            );
            (
                Encoding::D,
                "30-byte index, sampling read failed; assuming 8-byte records".to_owned(),
            )
        }
        Ok(None) => {
            log::warn!(
                "function {}: record width not established from sampled \
                 scans of {}; assuming 8-byte records",
                function,
                dat_path.display()
            );
            (
                Encoding::D,
                "30-byte index, record width not established from sampled \
                 scans; assuming 8-byte records"
                    .to_owned(),
            )
        }
    }
}

/// Judge the DAT record width of a 30-byte-index function from up to
/// [`VARIANT_B_WIDTH_SAMPLES`] evenly spaced scans. Returns the width and the
/// scan that decided it, or `None` when no sampled scan decides it.
fn sample_variant_b_width(
    dat_path: &Path,
    records: &[crate::raw::index::ScanIndexB],
    dat_size: u64,
) -> crate::Result<Option<(u64, usize)>> {
    let n = records.len();
    let samples = n.min(VARIANT_B_WIDTH_SAMPLES);
    for j in 0..samples {
        let i = n * (2 * j + 1) / (2 * samples);
        let start = records[i].dat_offset;
        let end = records.get(i + 1).map_or(dat_size, |r| r.dat_offset);
        // Out-of-range offsets are reported when the scan is decoded.
        if start >= end || end > dat_size {
            continue;
        }
        let bytes = read_slice(dat_path, start, end - start)
            .map_err(|e| e.with_context(format!("sampling {}", dat_path.display())))?;
        if let Some(width) = variant_b_record_width(&bytes) {
            return Ok(Some((width, i)));
        }
    }
    Ok(None)
}

/// The 22-byte index is paired with 6-byte, 8-byte and 12-byte DAT records.
/// Consecutive offsets give a direct width check without reading peak data.
fn variant_a_record_width(records: &[crate::raw::index::ScanIndexA]) -> Option<u64> {
    records.windows(2).find_map(|pair| {
        let count = u64::from(pair[0].n_records);
        let bytes = u64::from(pair[1].dat_offset).checked_sub(u64::from(pair[0].dat_offset))?;
        if count == 0 || bytes == 0 || bytes % count != 0 {
            return None;
        }
        match bytes / count {
            width @ (6 | 8 | 12) => Some(width),
            _ => None,
        }
    })
}

/// One scan after decoding.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DecodedScan {
    pub function_index: u32,
    /// 0-based position within the function.
    pub scan_idx: usize,
    pub retention_time_min: f32,
    /// Calibrated m/z and intensity. Ion mobility is not decoded.
    pub spectrum: Spectrum,
    /// Per-scan collision energy (eV) from `_FUNCnnn.STS`'s "Collision
    /// Energy" channel, when the file is present and defines that channel.
    pub collision_energy_ev: Option<f64>,
    /// Per-scan ETD Fragmentation Mode from `_FUNCnnn.STS`'s "ETD
    /// Fragmentation Mode" channel (seq 121): `0` for CID, non-zero for
    /// ETD, `None` when the file is absent or doesn't define the channel.
    pub etd_fragmentation_mode: Option<f64>,
}

/// Resolve the byte slice for scan `scan_idx` within `entry`'s DAT file.
///
/// Returns `(offset, length, retention_time_min)`. Trailing scans take the
/// length implied by `entry.dat_size`.
///
/// `dat_offset` and (for Variant B) the next record's `dat_offset` are raw
/// fields read straight from the `.IDX` file. An offset past the end of the
/// paired `.DAT` file, or (Variant B) a next offset that goes backwards or
/// past the end, is rejected with an error instead of decoding bytes that do
/// not belong to the scan. Variant A's `length` is capped against
/// `entry.dat_size` (the real, already-known size of the `.DAT` file): an
/// IDX record claiming a scan larger than the DAT file that actually exists
/// must not be able to force an allocation sized from unvalidated
/// file-controlled values in `read_slice`.
fn scan_slice(entry: &FunctionEntry, scan_idx: usize) -> crate::Result<(u64, u64, f32)> {
    let (offset, length, retention_time_min) = match &entry.scan_index {
        ScanIndex::A(records) => {
            let rec = records.get(scan_idx).ok_or_else(|| {
                crate::Error::Parse(format!(
                    "function {} scan {} out of range",
                    entry.index, scan_idx
                ))
            })?;
            // Variant A stores the record count directly; the width depends
            // on the DAT layout (6 bytes for A, 8 for D, 12 for E).
            let offset = u64::from(rec.dat_offset);
            if offset > entry.dat_size {
                return Err(crate::Error::Parse(format!(
                    "function {} scan {scan_idx}: DAT offset {offset} is past the end of \
                     {}-byte DAT file",
                    entry.index, entry.dat_size
                )));
            }
            let width = match entry.encoding {
                Encoding::A => 6,
                Encoding::D => 8,
                Encoding::E => 12,
            };
            let length = (rec.n_records as u64) * width;
            (offset, length, rec.retention_time_min)
        }
        ScanIndex::B(records) => {
            let rec = records.get(scan_idx).ok_or_else(|| {
                crate::Error::Parse(format!(
                    "function {} scan {} out of range",
                    entry.index, scan_idx
                ))
            })?;
            let offset = rec.dat_offset;
            let next_offset = records
                .get(scan_idx + 1)
                .map(|r| r.dat_offset)
                .unwrap_or(entry.dat_size);
            if offset > entry.dat_size || next_offset > entry.dat_size {
                return Err(crate::Error::Parse(format!(
                    "function {} scan {scan_idx}: DAT offsets {offset}..{next_offset} \
                     extend past the end of {}-byte DAT file",
                    entry.index, entry.dat_size
                )));
            }
            if next_offset < offset {
                return Err(crate::Error::Parse(format!(
                    "function {} scan {scan_idx}: next scan's DAT offset {next_offset} \
                     is before this scan's offset {offset}",
                    entry.index
                )));
            }
            (offset, next_offset - offset, rec.retention_time_min)
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
            Encoding::D => "D",
            Encoding::E => "E",
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
        let encoding = if matches!(scan_index, ScanIndex::A(_)) {
            Encoding::A
        } else {
            Encoding::D
        };
        FunctionEntry {
            index: 1,
            info: dummy_info(1),
            scan_index,
            dat_path: PathBuf::new(),
            dat_size,
            encoding,
            cal: FunctionCal::default(),
            sts: None,
        }
    }

    // A corrupt/malicious IDX can claim a scan far larger than the real DAT
    // file: dat_offset=0 for this scan, dat_offset=u32::MAX-1 for the "next"
    // scan used to compute Variant B's length by subtraction. `scan_slice`
    // must reject it: passing that difference (up to ~4.29 GB) to `read_slice`
    // would allocate a `Vec` sized from it regardless of how small the real
    // DAT file is, which aborts the process under a virtual-memory limit
    // rather than returning a recoverable error.
    #[test]
    fn variant_b_next_offset_beyond_dat_size_is_error() {
        let entry = entry_with(
            ScanIndex::B(vec![
                ScanIndexB {
                    dat_offset: 0,
                    retention_time_min: 0.0,
                },
                ScanIndexB {
                    dat_offset: u64::from(u32::MAX - 1),
                    retention_time_min: 0.1,
                },
            ]),
            64, // real DAT file is tiny
        );
        let error = scan_slice(&entry, 0).unwrap_err().to_string();
        assert!(error.contains("past the end"), "{error}");
    }

    #[test]
    fn variant_b_offset_beyond_dat_size_is_error() {
        let entry = entry_with(
            ScanIndex::B(vec![ScanIndexB {
                dat_offset: 1_000_000,
                retention_time_min: 0.0,
            }]),
            64,
        );
        let error = scan_slice(&entry, 0).unwrap_err().to_string();
        assert!(error.contains("past the end"), "{error}");
    }

    #[test]
    fn variant_b_backwards_offset_is_error() {
        let entry = entry_with(
            ScanIndex::B(vec![
                ScanIndexB {
                    dat_offset: 40,
                    retention_time_min: 0.0,
                },
                ScanIndexB {
                    dat_offset: 8,
                    retention_time_min: 0.1,
                },
            ]),
            100,
        );
        let error = scan_slice(&entry, 0).unwrap_err().to_string();
        assert!(error.contains("before this scan's offset"), "{error}");
        // The last scan runs to EOF and is still readable.
        assert_eq!(scan_slice(&entry, 1).unwrap(), (8, 92, 0.1));
    }

    // Offsets above 4 GiB (high word at IDX +0x1A) must be used as-is, not
    // truncated to 32 bits, so the slice lands in the right place of a large
    // DAT file.
    #[test]
    fn variant_b_offsets_above_four_gib_are_used_in_full() {
        let base = (1u64 << 32) + 1_626_312;
        let entry = entry_with(
            ScanIndex::B(vec![
                ScanIndexB {
                    dat_offset: base,
                    retention_time_min: 0.0,
                },
                ScanIndexB {
                    dat_offset: base + 800,
                    retention_time_min: 0.1,
                },
            ]),
            base + 1000,
        );
        assert_eq!(scan_slice(&entry, 0).unwrap(), (base, 800, 0.0));
        assert_eq!(scan_slice(&entry, 1).unwrap(), (base + 800, 200, 0.1));
    }

    #[test]
    fn variant_a_offset_beyond_dat_size_is_error() {
        let entry = entry_with(
            ScanIndex::A(vec![ScanIndexA {
                dat_offset: 1_000,
                n_records: 1,
                retention_time_min: 0.0,
                peak_count: 0,
            }]),
            64,
        );
        assert!(scan_slice(&entry, 0).is_err());
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
                n_records: u16::MAX.into(), // claims 393,210 bytes
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

    #[test]
    fn variant_a_twelve_byte_records_are_recognized() {
        let records = vec![
            ScanIndexA {
                dat_offset: 0,
                n_records: 2,
                retention_time_min: 0.0,
                peak_count: 0,
            },
            ScanIndexA {
                dat_offset: 24,
                n_records: 3,
                retention_time_min: 0.1,
                peak_count: 0,
            },
        ];
        assert_eq!(variant_a_record_width(&records), Some(12));
        let mut entry = entry_with(ScanIndex::A(records), 60);
        entry.encoding = Encoding::E;
        assert_eq!(scan_slice(&entry, 0).unwrap().1, 24);
        assert_eq!(scan_slice(&entry, 1).unwrap().1, 36);
        assert_eq!(index_end(&entry.scan_index, entry.encoding), Some(60));
    }

    #[test]
    fn variant_a_eight_byte_records_use_index_count() {
        let records = vec![
            ScanIndexA {
                dat_offset: 0,
                n_records: 5,
                retention_time_min: 0.0,
                peak_count: 0,
            },
            ScanIndexA {
                dat_offset: 40,
                n_records: 2,
                retention_time_min: 0.1,
                peak_count: 0,
            },
        ];
        assert_eq!(variant_a_record_width(&records), Some(8));
        let mut entry = entry_with(ScanIndex::A(records), 56);
        entry.encoding = Encoding::D;
        assert_eq!(scan_slice(&entry, 0).unwrap().1, 40);
        assert_eq!(scan_slice(&entry, 1).unwrap().1, 16);
    }

    #[test]
    fn open_missing_directory_names_the_path() {
        let error = Reader::open("/nonexistent/example.raw")
            .unwrap_err()
            .to_string();
        assert!(error.contains("/nonexistent/example.raw"), "{error}");
    }

    #[test]
    fn decode_error_identifies_bundle_function_and_scan() {
        let reader = Reader {
            dir: PathBuf::from("example.raw"),
            bundle_name: "example.raw".into(),
            header: Header::default(),
            extern_inf: "Lteff 2200\nVeff 5000".parse().unwrap(),
            functions: Vec::new(),
        };
        let error = reader.decode_scan(4, 7).unwrap_err().to_string();
        assert!(error.contains("example.raw function 4 scan 7"));
        assert!(error.contains("function 4 not present"));
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
    fn peak_count_sanity_does_not_panic_when_peak_count_exceeds_decoded_len() {
        // File-controlled value: a mismatch is logged, never a panic, in
        // debug and release builds alike.
        check_peak_count_sanity(1, 0, 100, 5);
    }

    // A DAT file that passes the size check but cannot be read while sampling
    // must not fail `Reader::open` for the whole bundle.
    #[test]
    fn variant_b_sampling_read_failure_assumes_encoding_d() {
        let records = vec![ScanIndexB {
            dat_offset: 0,
            retention_time_min: 0.0,
        }];
        let missing = std::env::temp_dir().join("openwraw-test-missing/_FUNC001.DAT");
        let (encoding, reason) = variant_b_encoding(1, &missing, &records, 64);
        assert!(matches!(encoding, Encoding::D));
        assert!(reason.contains("sampling read failed"), "{reason}");
    }
}
