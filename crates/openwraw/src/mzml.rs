//! mzML export for Waters `.raw/` bundles, built on the canonical writer
//! in `openmassspec-core`.
//!
//! Frame -> spectrum projection:
//!
//! * One mzML spectrum per scan in each non-lock-mass function.
//! * Every encoding: the decoded m/z and intensity points are emitted as-is.
//!   No lock-mass correction is applied, and ion mobility is not decoded,
//!   so spectra carry no mobility array and the run declares no mobility
//!   array kind, including for SYNAPT mobility acquisitions.
//! * Native ID format mirrors the de-facto Waters convention used by
//!   ProteoWizard / Wiff2: `function=F process=0 scan=S` (1-based S).
//! * Lock-mass / reference functions are skipped.

use std::io::Write;
use std::path::Path;

use openmassspec_core as msc;

use crate::raw::chroms::{read_chro_dat, ChromsInf};
use crate::reader::{find_file, DecodedScan, Reader};

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
/// Every entry here is checked directly against psi-ms.obo.
///
/// `SYNAPT G2-S`, `SYNAPT G2`, and bare `SYNAPT` are deliberately *not* in
/// this table: the real CV only defines "HDMS" and "MS" (non-HDMS)
/// variants for each of those models (e.g. `Synapt G2-S HDMS` vs.
/// `Synapt G2-S MS`), and the header string alone doesn't say which
/// acquisition mode a given file used - picking one would be a guess, not
/// a verified mapping. They fall through to the generic Waters term below.
///
/// Investigated (Sigilweaver/OpenWRaw#11) and ruled out as disambiguating
/// signals:
/// - `_FUNCnnn.IDX` record variant (A vs B): Variant B is shared by IMS
///   (SYNAPT) and non-IMS (Xevo G2-XS) instruments, so it tells us nothing
///   about HDMS-vs-MS mode even for models it does apply to (see
///   `docs/format/03-func-idx.md`, "Distinguishing Variant A from B").
/// - `Apex3DIons.csv` / `Apex3Dnnn.bin` presence: only written when the
///   optional Apex3D post-processing module was run, so presence weakly
///   implies IMS but absence proves nothing (see
///   `docs/format/11-apex3d-bin.md`).
/// - `_extern.inf` pusher fields (`PusherInterval` / `Pusher Cycle Time`):
///   present on both IMS and non-IMS Q-Tof/Synapt instruments alike (pusher
///   is a standard orthogonal-acceleration TOF component, not IMS-specific).
///
/// No sample files for `SYNAPT G2-S`, `SYNAPT G2`, or bare `SYNAPT` exist in
/// the corpus to test any of this against empirically (the corpus's one
/// bare-QTOF sample, PXD058812, reports `Instrument: QTOF` in `_HEADER.TXT`,
/// not a Synapt string at all - an older Q-Tof-family unit, not a Synapt
/// running in MS mode). Absent a real HDMS-mode and MS-mode pair of files
/// from the same model to compare, this is a data-availability gap, not
/// something more code-reading can resolve. The generic Waters term is the
/// correct permanent answer here unless real sample files surface.
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
    let mut extra = ::std::collections::BTreeMap::new();
    if let Some(value) = &reader.header.version {
        extra.insert("openwraw.header_version".into(), value.clone());
    }
    if let Some(value) = &reader.header.acquired_name {
        extra.insert("openwraw.acquired_name".into(), value.clone());
    }
    if let Some(value) = &reader.header.acquired_date {
        extra.insert("openwraw.acquired_date".into(), value.clone());
    }
    if let Some(value) = &reader.header.acquired_time {
        extra.insert("openwraw.acquired_time".into(), value.clone());
    }
    if let Some(value) = &reader.header.operator {
        extra.insert("openwraw.operator".into(), value.clone());
    }
    if let Some(value) = &reader.header.sample_description {
        extra.insert("openwraw.sample_description".into(), value.clone());
    }
    extra.insert(
        "openwraw.lteff_mm".into(),
        reader.extern_inf.lteff_mm.to_string(),
    );
    extra.insert(
        "openwraw.veff_v".into(),
        reader.extern_inf.veff_v.to_string(),
    );
    if let Some(value) = reader.extern_inf.pusher_interval_us {
        extra.insert("openwraw.pusher_interval_us".into(), value.to_string());
    }
    for (index, function) in &reader.extern_inf.functions {
        let prefix = format!("openwraw.function.{index}");
        extra.insert(format!("{prefix}.mode"), format!("{:?}", function.mode));
        if let Some(value) = function.pusher_interval_us {
            extra.insert(format!("{prefix}.pusher_interval_us"), value.to_string());
        }
        if let Some(value) = function.set_mass_da {
            extra.insert(format!("{prefix}.set_mass_da"), value.to_string());
        }
    }
    for (index, cal) in &reader.header.cal_functions {
        let prefix = format!("openwraw.calibration.{index}");
        extra.insert(format!("{prefix}.type"), format!("{:?}", cal.cal_type));
        extra.insert(
            format!("{prefix}.coefficients"),
            cal.coeffs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
    }
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
        extra,
        source_file_name: reader.bundle_name.clone(),
        source_file_format: source_file_format_cv(),
        native_id_format: native_id_format_cv(),
        instrument: instrument_cv(&instrument_name),
        instrument_serial_number: None,
        software_name: SOFTWARE_NAME.into(),
        software_version: SOFTWARE_VERSION.into(),
        acquisition_software_name: None,
        acquisition_software_version: None,
        start_timestamp,
        // Ion mobility is not decoded, so no spectrum carries a mobility array.
        mobility_array_kind: None,
        analyzers: Vec::new(),
    }
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

/// Build a spectrum's `PrecursorInfo`, or `None` for MS1 (and any other
/// function this reader has no precursor signal for at all).
///
/// Two independent, corpus-verified sources feed this (Sigilweaver/OpenWRaw#8,
/// #13):
/// * `target_mz` from `_extern.inf`'s `Set Mass` field, present only on
///   `TOF MSMS FUNCTION` / `TOF DAUGHTER FUNCTION` sections (real targeted
///   MS/MS - a fixed precursor list or SRM-like acquisition). `TOF PARENT
///   FUNCTION` (MSe/HDMSe) sections never have this field: broadband
///   `Precursor Selection: Everything` fragmentation has no discrete
///   precursor to set, so `None` here is correct, not a decoding gap.
/// * `collision_energy` from `_FUNCnnn.STS`'s per-scan "Collision Energy"
///   channel, which is independent of `target_mz` and is populated for MSe
///   functions too (Waters records a real collision energy for the
///   high-energy MSe scan even though it has no discrete precursor).
///
/// Returns `None` (rather than `Some` with every field `None`) when neither
/// source has anything to report, so a spectrum with genuinely no precursor
/// information has no `<precursor>` element.
///
/// No charge state or isolation width field has been found in either file
/// for any corpus sample (targeted or MSe); this is a real gap in what
/// `_extern.inf`/`_FUNCnnn.STS` expose, not an oversight in this function.
fn precursor_info_for(
    reader: &Reader,
    function_index: u32,
    ms_level: u32,
    collision_energy_ev: Option<f64>,
    etd_fragmentation_mode: Option<f64>,
) -> Option<msc::PrecursorInfo> {
    if ms_level < 2 {
        return None;
    }
    let target_mz = reader
        .extern_inf
        .functions
        .get(&function_index)
        .and_then(|f| f.set_mass_da);
    if target_mz.is_none() && collision_energy_ev.is_none() {
        return None;
    }
    Some(msc::PrecursorInfo {
        target_mz,
        collision_energy: collision_energy_ev,
        ce_is_nce: false,
        activation: activation_for(etd_fragmentation_mode),
        ..Default::default()
    })
}

/// Map `_FUNCnnn.STS`'s "ETD Fragmentation Mode" channel (seq 121) to an
/// `Activation` variant per `docs/docs/format/07-func-sts.md`: `0` is CID,
/// non-zero is ETD. `None` when the channel isn't present in the STS file.
fn activation_for(etd_fragmentation_mode: Option<f64>) -> Option<msc::Activation> {
    etd_fragmentation_mode.map(|v| {
        if v == 0.0 {
            msc::Activation::CID
        } else {
            msc::Activation::ETD
        }
    })
}

/// Map a `_CHROMS.INF` channel's engineering units to a PSI-MS chromatogram
/// type term, verified against psi-ms.obo.
///
/// Only units with an exact, unambiguous CV match are mapped. Channels like
/// "BSM Composition B" (%) or "(1) Peltier Engine Power" (% Power) have no
/// corresponding PSI-MS chromatogram-type term (checked: the only children of
/// `MS:1000626` "chromatogram type" are ion-current/electromagnetic-radiation
/// variants plus temperature/pressure/flow-rate) - those channels are left
/// out of [`chromatogram_records_for`] rather than mislabeled or defaulted to
/// "total ion current chromatogram".
fn chromatogram_type_for_units(units: &str) -> Option<msc::CvTerm> {
    let u = units.trim();
    if u.ends_with("/min") {
        return Some(msc::CvTerm::new("MS:1003020", "flow rate chromatogram"));
    }
    if u.eq_ignore_ascii_case("psi")
        || u.eq_ignore_ascii_case("bar")
        || u.eq_ignore_ascii_case("kpa")
    {
        return Some(msc::CvTerm::new("MS:1003019", "pressure chromatogram"));
    }
    if u.contains('\u{00B0}') {
        // Degree sign present -> temperature, whether °C or °F.
        return Some(msc::CvTerm::new("MS:1002715", "temperature chromatogram"));
    }
    None
}

/// Decode every mappable instrument channel in `_CHROMS.INF`/`_CHROnnnn.DAT`
/// into `openmassspec_core` chromatogram records.
///
/// Returns an empty vec (not an error) when `_CHROMS.INF` is absent, per its
/// own docs: direct-infusion / pure-MS bundles don't record LC channels.
/// A `_CHROMS.INF`, channel, or companion `.DAT` file that cannot be read
/// is logged at warn level and skipped rather than aborting the whole run,
/// matching `iter_spectra`'s skip-and-log contract.
fn chromatogram_records_for(dir: &Path) -> Vec<msc::ChromatogramRecord> {
    let inf_path = match find_file(dir, "_CHROMS.INF") {
        Ok(Some(path)) => path,
        Ok(None) => return Vec::new(),
        Err(e) => {
            log::warn!("{}: cannot look up _CHROMS.INF: {e}", dir.display());
            return Vec::new();
        }
    };
    let inf = match ChromsInf::from_path(&inf_path) {
        Ok(inf) => inf,
        Err(e) => {
            log::warn!("{}: skipping chromatograms: {e}", inf_path.display());
            return Vec::new();
        }
    };

    let mut out = Vec::new();
    for ch in &inf.channels {
        let Some(chromatogram_type) = chromatogram_type_for_units(&ch.units) else {
            continue;
        };
        let chro_num = inf.chro_number_for_channel(ch.index);
        let dat_name = format!("_CHRO{chro_num:03}.DAT");
        let dat_path = match find_file(dir, &dat_name) {
            Ok(Some(path)) => path,
            Ok(None) => {
                log::warn!(
                    "{}: channel {} ({}): {dat_name} not found, skipping",
                    dir.display(),
                    ch.index,
                    ch.name
                );
                continue;
            }
            Err(e) => {
                log::warn!(
                    "{}: channel {} ({}): cannot look up {dat_name}: {e}",
                    dir.display(),
                    ch.index,
                    ch.name
                );
                continue;
            }
        };
        let points = match read_chro_dat(&dat_path) {
            Ok(points) => points,
            Err(e) => {
                log::warn!(
                    "{}: channel {} ({}) skipped: {e}",
                    dat_path.display(),
                    ch.index,
                    ch.name
                );
                continue;
            }
        };
        let scale = ch.scale_f as f32;
        let time_sec = points.iter().map(|p| p.rt_min * 60.0).collect();
        let intensity = points.iter().map(|p| p.value * scale).collect();
        out.push(msc::ChromatogramRecord {
            index: out.len(),
            id: ch.name.clone(),
            chromatogram_type: Some(chromatogram_type),
            precursor_mz: None,
            product_mz: None,
            time_sec,
            intensity,
        });
    }
    out
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
pub fn record_from_scan(
    reader: &Reader,
    scan_counter: u32,
    scan: DecodedScan,
) -> msc::SpectrumRecord {
    let DecodedScan {
        function_index,
        scan_idx,
        retention_time_min,
        spectrum,
        collision_energy_ev,
        etd_fragmentation_mode,
    } = scan;
    let (mz, intensity) = (spectrum.mz, spectrum.intensity);
    let (tic, bp_mz, bp_int, low_mz, high_mz) = summarize_arrays(&mz, &intensity);
    let ms_level = ms_level_for_function(reader, function_index);
    let precursor = precursor_info_for(
        reader,
        function_index,
        ms_level,
        collision_energy_ev,
        etd_fragmentation_mode,
    );
    let mut extra = ::std::collections::BTreeMap::new();
    extra.insert("openwraw.function_index".into(), function_index.to_string());
    extra.insert("openwraw.scan_index".into(), scan_idx.to_string());
    if let Some(function) = reader.functions.iter().find(|f| f.index == function_index) {
        if let Some(sts) = &function.sts {
            for channel in sts.channels() {
                if let Some(value) = sts.value_at(channel, scan_idx) {
                    extra.insert(
                        format!("openwraw.sts.{}.{}", channel.seq, channel.name),
                        value.to_string(),
                    );
                }
            }
        }
    }
    msc::SpectrumRecord {
        extra,
        acquisition_event_id: None,
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
        precursor,
        mz,
        intensity,
        inv_mobility_per_peak: None,
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
    /// Scans that fail to decode are skipped. Each failure is logged at warn
    /// level, and the total is logged once the iterator is exhausted.
    fn iter_spectra<'s>(&'s mut self) -> Box<dyn Iterator<Item = msc::SpectrumRecord> + 's> {
        let reader = &self.reader;
        let mut scan_counter: u32 = 0;
        let skipped = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let skipped_in_loop = std::rc::Rc::clone(&skipped);
        let records = reader.iter_spectra().filter_map(move |decoded| {
            scan_counter += 1;
            match decoded {
                Ok(scan) => Some(record_from_scan(reader, scan_counter, scan)),
                Err(e) => {
                    skipped_in_loop.set(skipped_in_loop.get() + 1);
                    log::warn!("skipping scan {scan_counter}: {e}");
                    None
                }
            }
        });
        let summary = std::iter::once_with(move || {
            if skipped.get() > 0 {
                log::warn!(
                    "{}: {} scan(s) failed to decode and were skipped",
                    reader.bundle_name,
                    skipped.get()
                );
            }
            None
        })
        .flatten();
        Box::new(records.chain(summary))
    }
    fn spectrum_count_hint(&self) -> Option<usize> {
        Some(self.reader.total_scan_count())
    }
    fn iter_chromatograms<'s>(
        &'s mut self,
    ) -> Box<dyn Iterator<Item = msc::ChromatogramRecord> + 's> {
        Box::new(chromatogram_records_for(&self.reader.dir).into_iter())
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
    use crate::reader::Encoding;

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

    // Sigilweaver/OpenWRaw#22: no corpus fixture has a non-zero "ETD
    // Fragmentation Mode" value, so the non-zero -> ETD branch can only be
    // exercised synthetically here rather than against real acquisition
    // data.
    #[test]
    fn activation_for_maps_zero_to_cid_and_nonzero_to_etd() {
        assert_eq!(activation_for(Some(0.0)), Some(msc::Activation::CID));
        assert_eq!(activation_for(Some(1.0)), Some(msc::Activation::ETD));
        assert_eq!(activation_for(Some(-1.0)), Some(msc::Activation::ETD));
        assert_eq!(activation_for(None), None);
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

    // Regression test: every (units, accession) pair here was checked
    // directly against psi-ms.obo's children of MS:1000626 "chromatogram
    // type" (only ion-current/electromagnetic-radiation plus
    // temperature/pressure/flow-rate exist - there is no CV term for e.g.
    // solvent composition % or heater power %, so those must resolve to
    // `None` rather than being mislabeled).
    #[test]
    fn chromatogram_type_for_units_resolves_known_units_to_correct_psi_ms_accessions() {
        let cases = [
            (
                "\u{00B5}L/min",
                Some(("MS:1003020", "flow rate chromatogram")),
            ),
            ("mL/min", Some(("MS:1003020", "flow rate chromatogram"))),
            ("psi", Some(("MS:1003019", "pressure chromatogram"))),
            ("bar", Some(("MS:1003019", "pressure chromatogram"))),
            (
                "\u{00B0}C",
                Some(("MS:1002715", "temperature chromatogram")),
            ),
            (
                "\u{00B0}F",
                Some(("MS:1002715", "temperature chromatogram")),
            ),
            ("%", None),
            ("% Power", None),
        ];
        for (units, expected) in cases {
            let got = chromatogram_type_for_units(units);
            match expected {
                Some((acc, name)) => {
                    let cv = got.unwrap_or_else(|| panic!("expected a CV term for {units:?}"));
                    assert_eq!(cv.accession, acc, "wrong accession for units {units:?}");
                    assert_eq!(cv.name, name, "wrong CV name for units {units:?}");
                }
                None => assert!(got.is_none(), "expected no CV term for units {units:?}"),
            }
        }
    }

    // Sigilweaver/OpenWRaw#8 / #13: targeted MS/MS ("TOF MSMS FUNCTION", Set
    // Mass = 884.9) must populate `precursor.target_mz`, unlike broadband
    // MSe/HDMSe acquisitions, which have no discrete precursor at all.
    #[test]
    fn corpus_pxd035818_targeted_msms_populates_precursor_target_mz() {
        let Some(dir) =
            crate::test_corpus::bundle(&["PXD035818/17122018_TNFA_PEPTIDE_GSHH_MSMS_884.raw"])
        else {
            return;
        };
        let reader = Reader::open(&dir).unwrap();
        let records = collect_records(&reader).unwrap();
        assert!(!records.is_empty());
        for rec in &records {
            assert_eq!(rec.ms_level, 2, "every scan in this bundle is MS/MS");
            let precursor = rec.precursor.as_ref().unwrap_or_else(|| {
                panic!("scan {} missing precursor info entirely", rec.scan_number)
            });
            let mz = precursor
                .target_mz
                .unwrap_or_else(|| panic!("scan {} missing target_mz", rec.scan_number));
            assert!(
                (mz - 884.9).abs() < 1e-6,
                "scan {}: target_mz = {mz}, expected 884.9",
                rec.scan_number
            );
            assert!(!precursor.ce_is_nce);
        }
    }

    // Sigilweaver/OpenWRaw#8: broadband HDMSe has no discrete precursor
    // (`Precursor Selection: Everything`), so target_mz must stay None even
    // though collision_energy is available from `_FUNCnnn.STS`.
    #[test]
    fn corpus_pxd075602_hdmse_has_collision_energy_but_no_target_mz() {
        let Some(dir) = crate::test_corpus::bundle(&["PXD075602/DHPR_11257-1.raw"]) else {
            return;
        };
        let reader = Reader::open(&dir).unwrap();
        let records = collect_records(&reader).unwrap();
        let ms2: Vec<_> = records.iter().filter(|r| r.ms_level == 2).collect();
        assert!(!ms2.is_empty(), "expected at least one MSe MS2 scan");
        for rec in &ms2 {
            let precursor = rec
                .precursor
                .as_ref()
                .unwrap_or_else(|| panic!("scan {} missing precursor info", rec.scan_number));
            assert!(
                precursor.target_mz.is_none(),
                "HDMSe scan {} should have no discrete target_mz",
                rec.scan_number
            );
            assert!(
                precursor.collision_energy.is_some(),
                "HDMSe scan {} should still report collision_energy from _FUNCnnn.STS",
                rec.scan_number
            );
        }
    }

    fn reader_with_functions(functions: &[(Encoding, u8)]) -> Reader {
        use crate::raw::functions_inf::FunctionInfo;
        use crate::raw::index::ScanIndex;
        use crate::reader::FunctionEntry;
        let functions = functions
            .iter()
            .enumerate()
            .map(|(i, &(encoding, scan_subtype))| FunctionEntry {
                index: i as u32 + 1,
                info: FunctionInfo {
                    index: i as u32 + 1,
                    function_type: 0,
                    scan_subtype,
                    cycle_time_s: 0.0,
                    interscan_delay_s: 0.0,
                    scan_time_s: 0.0,
                    tof_depth: 0,
                    mz_low: 0.0,
                    mz_high: 0.0,
                },
                scan_index: ScanIndex::B(Vec::new()),
                dat_path: std::path::PathBuf::new(),
                dat_size: 0,
                encoding,
                cal: Default::default(),
                sts: None,
            })
            .collect();
        Reader {
            dir: std::path::PathBuf::from("example.raw"),
            bundle_name: "example.raw".into(),
            header: Default::default(),
            extern_inf: "Lteff 2200\nVeff 5000".parse().unwrap(),
            functions,
        }
    }

    #[test]
    fn source_skips_scans_that_fail_to_decode() {
        use crate::raw::index::{ScanIndex, ScanIndexB};
        use msc::SpectrumSource;
        let mut reader = reader_with_functions(&[(Encoding::D, 0)]);
        // Offset past the end of a 0-byte DAT: scan_slice rejects it.
        reader.functions[0].scan_index = ScanIndex::B(vec![ScanIndexB {
            dat_offset: 100,
            retention_time_min: 0.0,
        }]);
        assert!(reader.decode_scan(1, 0).is_err());
        let mut source = WatersSource::new(reader);
        assert_eq!(source.iter_spectra().count(), 0);
    }

    #[test]
    fn no_mobility_array_kind_is_declared() {
        // Ion mobility is not decoded, so no run declares a mobility array
        // kind, whatever its encodings or instrument.
        let kind = |functions: &[(Encoding, u8)]| {
            run_metadata_for(&reader_with_functions(functions)).mobility_array_kind
        };
        assert_eq!(kind(&[(Encoding::D, 0), (Encoding::D, 0x80)]), None);
        assert_eq!(kind(&[(Encoding::A, 0), (Encoding::E, 0)]), None);
    }

    #[test]
    fn chromatogram_records_for_empty_when_chroms_inf_absent() {
        let dir = std::env::temp_dir().join("openwraw-test-no-chroms-inf");
        let _ = std::fs::create_dir_all(&dir);
        let records = chromatogram_records_for(&dir);
        assert!(records.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Build a minimal, valid synthetic `.raw/` chromatogram pair: one
    /// mappable channel (flow rate) and one unmappable channel (%
    /// composition), matching the byte layouts documented in
    /// `raw::chroms` and its own corpus-derived tests.
    #[test]
    fn chromatogram_records_for_parses_synthetic_channels_and_skips_unmapped_units() {
        let dir = std::env::temp_dir().join("openwraw-test-synthetic-chroms");
        std::fs::create_dir_all(&dir).unwrap();

        // -- _CHROMS.INF: descriptor header + 2 channel records --
        const RECORD_SIZE: usize = 85;
        let mut inf = vec![0u8; 128];
        inf[0..2].copy_from_slice(&128u16.to_le_bytes());
        inf[2..4].copy_from_slice(&1u16.to_le_bytes());
        inf[4..6].copy_from_slice(&(RECORD_SIZE as u16).to_le_bytes());
        inf[6..8].copy_from_slice(&2u16.to_le_bytes());

        inf[32..38].copy_from_slice(&[1, 0, 2, 0, 0, 0]);
        inf[38..43].copy_from_slice(b"Flags");
        inf[64..66].copy_from_slice(&4u16.to_le_bytes());
        inf[80..86].copy_from_slice(&[2, 0, 5, 0, 4, 0]);
        inf[86..97].copy_from_slice(b"Description");
        inf[112..114].copy_from_slice(&81u16.to_le_bytes());

        let make_data = |source_type: u32, name: &str, cc: &str| {
            let mut r = vec![0u8; RECORD_SIZE];
            r[0..4].copy_from_slice(&source_type.to_le_bytes());
            let payload = &mut r[4..RECORD_SIZE];
            payload[..name.len()].copy_from_slice(name.as_bytes());
            let cc_start = name.len() + 1;
            payload[cc_start..cc_start + cc.len()].copy_from_slice(cc.as_bytes());
            r
        };
        // Channel 0: flow rate (mappable) -> _CHRO001.DAT
        inf.extend(make_data(4, "BSM Flow Rate A", "$CC$,1.0,3,0,0,mL/min"));
        // Channel 1: composition % (no CV term) -> _CHRO002.DAT, should be skipped
        inf.extend(make_data(4, "BSM Composition B", "$CC$,1.0,3,0,0,%"));

        std::fs::write(dir.join("_chroms.inf"), &inf).unwrap();

        let make_chro_dat = |points: &[(f32, f32)]| {
            let mut bytes = vec![0u8; 128];
            bytes[0..2].copy_from_slice(&128u16.to_le_bytes());
            bytes[2..4].copy_from_slice(&1u16.to_le_bytes());
            bytes[4..6].copy_from_slice(&8u16.to_le_bytes());
            bytes[6..8].copy_from_slice(&2u16.to_le_bytes());
            for &(rt, val) in points {
                bytes.extend_from_slice(&rt.to_le_bytes());
                bytes.extend_from_slice(&val.to_le_bytes());
            }
            bytes
        };
        std::fs::write(
            dir.join("_chro001.dat"),
            make_chro_dat(&[(0.0, 100.0), (0.5, 200.0)]),
        )
        .unwrap();
        std::fs::write(
            dir.join("_chro002.dat"),
            make_chro_dat(&[(0.0, 95.0), (0.5, 96.0)]),
        )
        .unwrap();

        let records = chromatogram_records_for(&dir);
        assert_eq!(records.len(), 1, "the % channel should be skipped");
        let rec = &records[0];
        assert_eq!(rec.id, "BSM Flow Rate A");
        assert_eq!(
            rec.chromatogram_type.as_ref().unwrap().accession,
            "MS:1003020"
        );
        assert_eq!(rec.time_sec, vec![0.0, 30.0]); // rt_min * 60
        assert_eq!(rec.intensity, vec![100.0, 200.0]); // scale_f = 1.0

        let _ = std::fs::remove_dir_all(&dir);
    }
}
