//! mzML export for Waters `.raw/` bundles, built on the canonical writer
//! in `mass-spec-core`.
//!
//! Frame -> spectrum projection:
//!
//! * One mzML spectrum per scan in each non-lock-mass function.
//! * Encoding A / C (non-IMS QTof): peaks are emitted as-is.
//! * Encoding B (SYNAPT IMS): all drift bins are pooled into a single MS
//!   spectrum per scan (sorted by m/z, intensities summed across drift
//!   bins). The drift dimension is dropped for now.
//! * Native ID format mirrors the de-facto Waters convention used by
//!   ProteoWizard / Wiff2: `function=F process=0 scan=S` (1-based S).
//! * Lock-mass / reference functions are skipped.

use std::io::Write;
use std::path::Path;

use mass_spec_core as msc;

use crate::raw::data::ImsSpectrum;
use crate::reader::{DecodedSpectrum, Reader};

const SOFTWARE_NAME: &str = "openwraw";
const SOFTWARE_VERSION: &str = env!("CARGO_PKG_VERSION");

fn source_file_format_cv() -> msc::CvTerm {
    // PSI-MS MS:1000526 = "Waters raw format".
    msc::CvTerm::new("MS:1000526", "Waters raw format")
}

fn native_id_format_cv() -> msc::CvTerm {
    // PSI-MS MS:1000769 = "Waters nativeID format".
    msc::CvTerm::new("MS:1000769", "Waters nativeID format")
}

fn instrument_cv(name: &str) -> msc::CvTerm {
    let up = name.to_ascii_uppercase();
    let known: &[(&str, &str, &str)] = &[
        ("SYNAPT G2-SI", "MS:1001769", "Synapt G2-Si"),
        ("SYNAPT G2-S", "MS:1001768", "Synapt G2-S"),
        ("SYNAPT G2", "MS:1001767", "Synapt G2"),
        ("SYNAPT", "MS:1001766", "Synapt MS"),
        ("XEVO G2-XS QTOF", "MS:1002472", "Xevo G2-XS QTof"),
        ("XEVO-G2XSQTOF", "MS:1002472", "Xevo G2-XS QTof"),
        ("XEVO G2 QTOF", "MS:1001790", "Xevo G2 QTof"),
        ("XEVO TQ-S", "MS:1001792", "Xevo TQ-S"),
        ("XEVO TQ", "MS:1001791", "Xevo TQ"),
        ("XEVO", "MS:1000533", "Xevo G2 Q-Tof"),
        ("QTOF", "MS:1000031", "Waters instrument model"),
    ];
    for (prefix, acc, term_name) in known {
        if up.starts_with(prefix) {
            return msc::CvTerm::new(acc, *term_name);
        }
    }
    msc::CvTerm::new("MS:1000126", "Waters instrument model")
}

fn polarity_for(_function_index: u32) -> Option<msc::Polarity> {
    // Waters polarity lives in _extern.inf per function; the current
    // ExternInf parser does not yet surface it. Leave unset rather than
    // guess.
    None
}

fn native_id_for(function_index: u32, scan_idx_zero_based: usize) -> String {
    format!(
        "function={function_index} process=0 scan={}",
        scan_idx_zero_based + 1
    )
}

/// Build a [`msc::RunMetadata`] from a [`Reader`].
fn run_metadata_for(reader: &Reader) -> msc::RunMetadata {
    let instrument_name = reader
        .header
        .instrument
        .clone()
        .unwrap_or_else(|| "Waters".into());
    let start_timestamp = reader
        .header
        .acquired_date
        .as_deref()
        .map(|d| format!("{d} {}", reader.header.acquired_time.as_deref().unwrap_or("")));
    msc::RunMetadata {
        source_file_name: reader.bundle_name.clone(),
        source_file_format: source_file_format_cv(),
        native_id_format: native_id_format_cv(),
        instrument: instrument_cv(&instrument_name),
        software_name: SOFTWARE_NAME.into(),
        software_version: SOFTWARE_VERSION.into(),
        start_timestamp,
    }
}

/// Pool an IMS scan's drift bins into a single MS spectrum.
///
/// Sorts the (m/z, intensity) pairs by m/z and sums intensities that fall
/// on the same m/z bin (after the encoder's 1/65536 sub-bin resolution).
fn pool_ims(ims: &ImsSpectrum) -> (Vec<f64>, Vec<f32>) {
    let n = ims.mz.len();
    let mut pairs: Vec<(f64, f32)> = Vec::with_capacity(n);
    for i in 0..n {
        pairs.push((ims.mz[i], ims.intensity[i]));
    }
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut mz: Vec<f64> = Vec::with_capacity(n);
    let mut intensity: Vec<f32> = Vec::with_capacity(n);
    for (m, i) in pairs {
        if let Some(last_m) = mz.last_mut() {
            if (*last_m - m).abs() < 1e-9 {
                if let Some(last_i) = intensity.last_mut() {
                    *last_i += i;
                    continue;
                }
            }
        }
        mz.push(m);
        intensity.push(i);
    }
    (mz, intensity)
}

fn ms_level_for_function(reader: &Reader, function_index: u32) -> u32 {
    // Without a robust MS/MS marker in the current _extern.inf parser,
    // treat function 1 as MS1 and every other survey function as MS2.
    if function_index == 1 || reader.functions.len() == 1 {
        1
    } else {
        2
    }
}

/// Collect every decoded scan in a bundle into `mass_spec_core` records.
fn collect_records(reader: &Reader) -> crate::Result<Vec<msc::SpectrumRecord>> {
    let mut out: Vec<msc::SpectrumRecord> = Vec::with_capacity(reader.total_scan_count());
    let mut scan_counter: u32 = 0;
    for decoded in reader.iter_spectra() {
        let scan = decoded?;
        scan_counter += 1;
        let (mz, intensity) = match &scan.spectrum {
            DecodedSpectrum::Plain(s) => (s.mz.clone(), s.intensity.clone()),
            DecodedSpectrum::Ims(ims) => pool_ims(ims),
        };
        let n = mz.len();
        let (tic, bp_mz, bp_int, low_mz, high_mz) = summarize_arrays(&mz, &intensity);
        let ms_level = ms_level_for_function(reader, scan.function_index);
        out.push(msc::SpectrumRecord {
            index: (scan_counter as usize).saturating_sub(1),
            scan_number: scan_counter,
            native_id: native_id_for(scan.function_index, scan.scan_idx),
            ms_level,
            polarity: polarity_for(scan.function_index),
            scan_mode: Some(msc::ScanMode::Centroid),
            analyzer: Some(msc::Analyzer::TOFMS),
            filter: None,
            retention_time_sec: scan.retention_time_min as f64 * 60.0,
            total_ion_current: Some(tic),
            base_peak_mz: bp_mz,
            base_peak_intensity: bp_int,
            low_mz,
            high_mz,
            ion_injection_time_ms: None,
            inv_mobility: None,
            precursor: None,
            mz,
            intensity,
            inv_mobility_per_peak: None,
        });
        let _ = n;
    }
    Ok(out)
}

fn summarize_arrays(
    mz: &[f64],
    intensity: &[f32],
) -> (f64, Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    if mz.is_empty() {
        return (0.0, None, None, None, None);
    }
    let mut tic: f64 = 0.0;
    let mut bp_int: f32 = 0.0;
    let mut bp_mz: f64 = mz[0];
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for (m, i) in mz.iter().zip(intensity.iter()) {
        tic += *i as f64;
        if *i > bp_int {
            bp_int = *i;
            bp_mz = *m;
        }
        if *m < lo {
            lo = *m;
        }
        if *m > hi {
            hi = *m;
        }
    }
    (
        tic,
        Some(bp_mz),
        Some(bp_int as f64),
        Some(lo),
        Some(hi),
    )
}

/// `SpectrumSource` adapter that owns a [`Reader`]. All spectra are decoded
/// up front (Waters bundles in the workspace corpus are <= 1GB so this is
/// fine; for larger bundles a streaming variant would be a worthwhile
/// follow-up).
pub struct WatersSource {
    reader: Reader,
    spectra: Option<Vec<msc::SpectrumRecord>>,
}

impl WatersSource {
    /// Build a source from an already-opened [`Reader`].
    pub fn new(reader: Reader) -> Self {
        Self {
            reader,
            spectra: None,
        }
    }

    /// Open a `.raw/` directory and wrap it in a source.
    pub fn open<P: AsRef<Path>>(dir: P) -> crate::Result<Self> {
        let reader = Reader::open(dir)?;
        Ok(Self::new(reader))
    }

    /// Reference to the underlying [`Reader`].
    pub fn reader(&self) -> &Reader {
        &self.reader
    }

    fn build_spectra(&mut self) -> crate::Result<&Vec<msc::SpectrumRecord>> {
        if self.spectra.is_none() {
            self.spectra = Some(collect_records(&self.reader)?);
        }
        Ok(self.spectra.as_ref().unwrap())
    }
}

impl msc::SpectrumSource for WatersSource {
    fn run_metadata(&self) -> msc::RunMetadata {
        run_metadata_for(&self.reader)
    }
    fn iter_spectra<'s>(&'s mut self) -> Box<dyn Iterator<Item = msc::SpectrumRecord> + 's> {
        let recs = self.build_spectra().cloned().unwrap_or_default();
        Box::new(recs.into_iter())
    }
    fn spectrum_count_hint(&self) -> Option<usize> {
        self.spectra.as_ref().map(|v| v.len())
    }
}

/// Convenience wrapper: open `dir`, decode every scan, emit mzML.
pub fn write_mzml<P: AsRef<Path>, W: Write>(dir: P, out: &mut W) -> crate::Result<()> {
    let mut src = WatersSource::open(dir)?;
    src.build_spectra()?;
    msc::write_mzml(&mut src, out).map_err(crate::Error::Io)?;
    Ok(())
}

/// Indexed-mzML equivalent of [`write_mzml`].
pub fn write_indexed_mzml<P: AsRef<Path>, W: Write>(dir: P, out: &mut W) -> crate::Result<()> {
    let mut src = WatersSource::open(dir)?;
    src.build_spectra()?;
    msc::write_indexed_mzml(&mut src, out).map_err(crate::Error::Io)?;
    Ok(())
}
