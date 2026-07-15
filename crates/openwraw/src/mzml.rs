//! mzML export for Waters `.raw/` bundles, built on the canonical writer
//! in `openmassspec-core`.
//!
//! Frame -> spectrum projection:
//!
//! * One mzML spectrum per scan in each non-lock-mass function.
//! * Encoding A / C (non-IMS QTof): peaks are emitted as-is.
//! * Encoding B (SYNAPT IMS): every drift bin contributes its own peak,
//!   with a parallel drift-time array emitted alongside m/z and intensity
//!   (MS:1003007 "raw ion mobility array", milliseconds). The `pool_ims`
//!   helper for m/z pooling stays available for downstream tools that
//!   want a single spectrum per scan.
//! * Native ID format mirrors the de-facto Waters convention used by
//!   ProteoWizard / Wiff2: `function=F process=0 scan=S` (1-based S).
//! * Lock-mass / reference functions are skipped.

use std::io::Write;
use std::path::Path;

use openmassspec_core as msc;

use crate::raw::data::ImsSpectrum;
use crate::reader::{DecodedScan, DecodedSpectrum, Reader};

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

/// Resolve a PSI-MS instrument CV term from the Waters `_HEADER.TXT`
/// `Instrument` field. Falls back to the generic Waters term when the model
/// string is unrecognized.
///
/// Every entry here was checked directly against psi-ms.obo (a prior
/// version of this table had several fabricated-looking accessions, e.g.
/// `XEVO G2-XS QTOF` pointed at `MS:1002472` = "trap-type
/// collision-induced dissociation", a completely unrelated CV category).
///
/// `SYNAPT G2-S`, `SYNAPT G2`, and bare `SYNAPT` are deliberately *not* in
/// this table: the real CV only defines "HDMS" and "MS" (non-HDMS)
/// variants for each of those models (e.g. `Synapt G2-S HDMS` vs.
/// `Synapt G2-S MS`), and the header string alone doesn't say which
/// acquisition mode a given file used - picking one would be a guess, not
/// a verified mapping. They fall through to the generic Waters term below
/// until that's resolved (tracked separately).
fn instrument_cv(name: &str) -> msc::CvTerm {
    let up = name.to_ascii_uppercase();
    let known: &[(&str, &str, &str)] = &[
        ("SYNAPT G2-SI", "MS:1002726", "SYNAPT G2-Si"),
        ("XEVO G2-XS QTOF", "MS:1003252", "Xevo G2-XS QTof"),
        ("XEVO-G2XSQTOF", "MS:1003252", "Xevo G2-XS QTof"),
        ("XEVO G2 QTOF", "MS:1001783", "Xevo G2 Q-Tof"),
        ("XEVO TQ-S", "MS:1001792", "Xevo TQ-S"),
        ("XEVO TQ", "MS:1001791", "Xevo TQD"),
    ];
    for (prefix, acc, term_name) in known {
        if up.starts_with(prefix) {
            return msc::CvTerm::new(acc, *term_name);
        }
    }
    msc::CvTerm::new("MS:1000126", "Waters instrument model")
}

fn polarity_for(reader: &Reader, _function_index: u32) -> Option<msc::Polarity> {
    // Waters records electrospray polarity once per run in _extern.inf.
    match reader.extern_inf.polarity {
        Some(crate::raw::extern_inf::Polarity::Positive) => Some(msc::Polarity::Positive),
        Some(crate::raw::extern_inf::Polarity::Negative) => Some(msc::Polarity::Negative),
        None => None,
    }
}

fn native_id_for(function_index: u32, scan_idx_zero_based: usize) -> String {
    format!(
        "function={function_index} process=0 scan={}",
        scan_idx_zero_based + 1
    )
}

/// Parse Waters' `Acquired Date` / `Acquired Time` header strings (e.g.
/// `"14-Jan-2021"` / `"16:20:52"`) into an RFC 3339 string, or `None` if
/// either doesn't match the expected format.
///
/// Like opentfraw's `acquisition_date_rfc3339`, the source value is the
/// instrument's local wall-clock time with no recorded timezone offset; the
/// trailing `Z` is a formatting convention, not a claim that this is a true
/// UTC instant.
fn parse_acquired_datetime(date: &str, time: &str) -> Option<String> {
    let mut d = date.splitn(3, '-');
    let day: u32 = d.next()?.parse().ok()?;
    let month = match d.next()?.to_ascii_lowercase().as_str() {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return None,
    };
    let year: u32 = d.next()?.parse().ok()?;
    if d.next().is_some() {
        return None;
    }

    let mut t = time.splitn(3, ':');
    let hour: u32 = t.next()?.parse().ok()?;
    let minute: u32 = t.next()?.parse().ok()?;
    let second: u32 = t.next()?.parse().ok()?;
    if t.next().is_some() {
        return None;
    }
    if !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }

    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
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
        .zip(reader.header.acquired_time.as_deref())
        .and_then(|(d, t)| parse_acquired_datetime(d, t));
    msc::RunMetadata {
        source_file_name: reader.bundle_name.clone(),
        source_file_format: source_file_format_cv(),
        native_id_format: native_id_format_cv(),
        instrument: instrument_cv(&instrument_name),
        software_name: SOFTWARE_NAME.into(),
        software_version: SOFTWARE_VERSION.into(),
        start_timestamp,
        mobility_array_kind: Some(msc::MobilityArrayKind::DriftTimeMilliseconds),
    }
}

/// Pool an IMS scan's drift bins into a single MS spectrum.
///
/// Sorts the (m/z, intensity) pairs by m/z and sums intensities that fall
/// on the same m/z bin (after the encoder's 1/65536 sub-bin resolution).
/// Available for downstream tools that want a single MS spectrum per scan;
/// the default export path emits the drift-resolved peaks instead.
pub fn pool_ims(ims: &ImsSpectrum) -> (Vec<f64>, Vec<f32>) {
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
    // Prefer unambiguous mode labels from the _extern.inf section
    // header; for `TOF PARENT` (which Waters uses for both low-energy
    // MS1 and high-energy fragment scans in MSe / HDMSe) and for
    // unknown labels, fall back to the function-index heuristic.
    use crate::raw::extern_inf::FunctionMode;
    if let Some(f) = reader.extern_inf.functions.get(&function_index) {
        match f.mode {
            FunctionMode::Ms | FunctionMode::Reference => return 1,
            FunctionMode::Msms | FunctionMode::Daughter => return 2,
            FunctionMode::MseParent | FunctionMode::Unknown => {}
        }
    }
    if function_index == 1 || reader.functions.len() == 1 {
        1
    } else {
        2
    }
}

/// Collect every decoded scan in a bundle into `openmassspec_core` records.
pub fn collect_records(reader: &Reader) -> crate::Result<Vec<msc::SpectrumRecord>> {
    let mut out: Vec<msc::SpectrumRecord> = Vec::with_capacity(reader.total_scan_count());
    let mut scan_counter: u32 = 0;
    for decoded in reader.iter_spectra() {
        let scan = decoded?;
        scan_counter += 1;
        out.push(record_from_scan(reader, scan_counter, scan));
    }
    Ok(out)
}

/// Convert one already-decoded scan into an `openmassspec_core` record.
///
/// `scan_counter` is the 1-based position of `scan` within the reader's
/// iteration order, not a count of successfully-decoded scans so far, so
/// `index`/`scan_number` stay stable regardless of whether earlier scans
/// failed to decode.
fn record_from_scan(reader: &Reader, scan_counter: u32, scan: DecodedScan) -> msc::SpectrumRecord {
    let DecodedScan {
        function_index,
        scan_idx,
        retention_time_min,
        spectrum,
    } = scan;
    let (mz, intensity, mobility) = match spectrum {
        DecodedSpectrum::Plain(s) => (s.mz, s.intensity, None),
        DecodedSpectrum::Ims(ims) => {
            let mob: Vec<f32> = ims.drift_time_ms.iter().map(|&d| d as f32).collect();
            (ims.mz, ims.intensity, Some(mob))
        }
    };
    let (tic, bp_mz, bp_int, low_mz, high_mz) = summarize_arrays(&mz, &intensity);
    let ms_level = ms_level_for_function(reader, function_index);
    msc::SpectrumRecord {
        index: (scan_counter as usize).saturating_sub(1),
        scan_number: scan_counter,
        native_id: native_id_for(function_index, scan_idx),
        ms_level,
        polarity: polarity_for(reader, function_index),
        scan_mode: Some(msc::ScanMode::Centroid),
        analyzer: Some(msc::Analyzer::TOFMS),
        filter: None,
        retention_time_sec: retention_time_min as f64 * 60.0,
        total_ion_current: Some(tic),
        base_peak_mz: bp_mz,
        base_peak_intensity: bp_int,
        low_mz,
        high_mz,
        ion_injection_time_ms: None,
        inv_mobility: None,
        faims_cv: None, // Waters instruments have no FAIMS interface.
        precursor: None,
        mz,
        intensity,
        inv_mobility_per_peak: mobility,
    }
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
    (tic, Some(bp_mz), Some(bp_int as f64), Some(lo), Some(hi))
}

/// `SpectrumSource` adapter that owns a [`Reader`]. Spectra are decoded
/// scan-by-scan as `iter_spectra` is driven; nothing is buffered beyond the
/// scan currently being yielded. A scan that fails to decode is skipped
/// (per [`msc::SpectrumSource::iter_spectra`]'s contract) rather than
/// aborting the whole run.
pub struct WatersSource {
    reader: Reader,
}

impl WatersSource {
    /// Build a source from an already-opened [`Reader`].
    pub fn new(reader: Reader) -> Self {
        Self { reader }
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
}

impl msc::SpectrumSource for WatersSource {
    fn run_metadata(&self) -> msc::RunMetadata {
        run_metadata_for(&self.reader)
    }
    fn iter_spectra<'s>(&'s mut self) -> Box<dyn Iterator<Item = msc::SpectrumRecord> + 's> {
        let reader = &self.reader;
        let mut scan_counter: u32 = 0;
        Box::new(reader.iter_spectra().filter_map(move |decoded| {
            scan_counter += 1;
            decoded
                .ok()
                .map(|scan| record_from_scan(reader, scan_counter, scan))
        }))
    }
    fn spectrum_count_hint(&self) -> Option<usize> {
        Some(self.reader.total_scan_count())
    }
}

/// Convenience wrapper: open `dir`, decode every scan, emit mzML.
pub fn write_mzml<P: AsRef<Path>, W: Write>(dir: P, out: &mut W) -> crate::Result<()> {
    let mut src = WatersSource::open(dir)?;
    msc::write_mzml(&mut src, out).map_err(crate::Error::Io)?;
    Ok(())
}

/// Indexed-mzML equivalent of [`write_mzml`].
pub fn write_indexed_mzml<P: AsRef<Path>, W: Write>(dir: P, out: &mut W) -> crate::Result<()> {
    let mut src = WatersSource::open(dir)?;
    msc::write_indexed_mzml(&mut src, out).map_err(crate::Error::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression test: every (name, accession) pair here was checked
    // directly against psi-ms.obo, not copied from the prior table (which
    // had several fabricated-looking accessions - e.g. bare "XEVO" pointed
    // at MS:1000533 = "Bioworks", unrelated Thermo software).
    #[test]
    fn instrument_cv_resolves_known_models_to_correct_psi_ms_accessions() {
        let cases = [
            ("SYNAPT G2-Si", "MS:1002726", "SYNAPT G2-Si"),
            ("Xevo G2-XS QTof", "MS:1003252", "Xevo G2-XS QTof"),
            ("Xevo G2 QTof", "MS:1001783", "Xevo G2 Q-Tof"),
            ("Xevo TQ-S", "MS:1001792", "Xevo TQ-S"),
            ("Xevo TQ", "MS:1001791", "Xevo TQD"),
            (
                "some future model nobody has heard of",
                "MS:1000126",
                "Waters instrument model",
            ),
        ];
        for (name, acc, term_name) in cases {
            let cv = instrument_cv(name);
            assert_eq!(cv.accession, acc, "wrong accession for {name:?}");
            assert_eq!(cv.name, term_name, "wrong CV name for {name:?}");
        }
    }

    #[test]
    fn parse_acquired_datetime_formats_rfc3339_with_trailing_z() {
        assert_eq!(
            parse_acquired_datetime("14-Jan-2021", "16:20:52"),
            Some("2021-01-14T16:20:52Z".into())
        );
    }

    #[test]
    fn parse_acquired_datetime_none_on_malformed_input() {
        assert_eq!(parse_acquired_datetime("not-a-date", "16:20:52"), None);
        assert_eq!(parse_acquired_datetime("14-Jan-2021", "not-a-time"), None);
        assert_eq!(parse_acquired_datetime("14-Xyz-2021", "16:20:52"), None);
        assert_eq!(parse_acquired_datetime("32-Jan-2021", "16:20:52"), None);
        assert_eq!(parse_acquired_datetime("14-Jan-2021", "25:00:00"), None);
    }
}
