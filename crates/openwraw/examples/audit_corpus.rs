//! Decode every non-lock-mass scan in a directory tree of Waters RAW bundles,
//! then check m/z accuracy on each lock-mass (reference) function.
//!
//! Usage: cargo run -p openwraw --release --example audit_corpus -- /path/to/corpus [--lock-only]
//!
//! `--lock-only` skips the full scan decode and runs only the lock-mass check.
//!
//! The reference mass comes from `ReferenceMass1` in `_extern.inf` when the
//! file declares one. Otherwise the common lock-mass compounds are tried:
//! leucine enkephalin [M+H]+ and [Glu1]-fibrinopeptide B [M+2H]2+. A lock
//! function fails when its median error exceeds `LOCK_TOLERANCE_PPM`.

use std::io::Write;
use std::path::{Path, PathBuf};

use openwraw::{DecodedSpectrum, FunctionEntry, Reader};

/// Correctly decoded lock functions in the corpus sit within about 70 ppm
/// before lock-mass correction (saturated reference peaks bias high);
/// decoder faults measured hundreds of ppm.
const LOCK_TOLERANCE_PPM: f64 = 100.0;
/// Lock scans sampled per function.
const LOCK_SCANS: usize = 20;
/// (m/z, charge) of leucine enkephalin [M+H]+ and [Glu1]-fibrinopeptide B
/// [M+2H]2+.
const LOCK_COMPOUNDS: [(f64, f64); 2] = [(556.2766, 1.0), (785.8421, 2.0)];
/// Profile points are summed into bins this wide (Da) before peak picking.
const BIN_DA: f64 = 0.002;

fn bundles(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut pending = vec![root.to_path_buf()];
    let mut found = Vec::new();
    while let Some(dir) = pending.pop() {
        let mut has_header = false;
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            // Skip macOS archive resource forks (`__MACOSX/._name` stubs).
            if kind.is_dir() && entry.file_name() != "__MACOSX" {
                pending.push(entry.path());
            } else if kind.is_file()
                && entry
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_uppercase()
                    .ends_with("_HEADER.TXT")
            {
                has_header = true;
            }
        }
        if has_header {
            found.push(dir);
        }
    }
    found.sort();
    Ok(found)
}

fn points(spectrum: DecodedSpectrum) -> (Vec<f64>, Vec<f32>) {
    match spectrum {
        DecodedSpectrum::Plain(s) => (s.mz, s.intensity),
        DecodedSpectrum::Ims(s) => (s.mz, s.intensity),
    }
}

/// `ReferenceMass1  1,556.27658` from the bundle's `_extern.inf`, if any.
fn declared_reference(dir: &Path) -> Option<f64> {
    let path = std::fs::read_dir(dir).ok()?.flatten().find(|e| {
        e.file_name()
            .to_string_lossy()
            .to_ascii_uppercase()
            .ends_with("_EXTERN.INF")
    })?;
    let text = String::from_utf8_lossy(&std::fs::read(path.path()).ok()?).into_owned();
    let line = text.lines().find(|l| l.starts_with("ReferenceMass1"))?;
    line.rsplit(',').next()?.trim().parse().ok()
}

type Bins = std::collections::BTreeMap<i64, (f64, f64)>;

/// Sum profile points within 2 Da of `reference` into `BIN_DA` bins of
/// (sum of m/z * intensity, sum of intensity). IMS drift cells collapse here.
fn bin_points(mz: &[f64], intensity: &[f32], reference: f64) -> Bins {
    let mut bins = Bins::new();
    for (&m, &i) in mz.iter().zip(intensity) {
        if (m - reference).abs() <= 2.0 {
            let bin = bins.entry((m / BIN_DA).floor() as i64).or_default();
            bin.0 += m * f64::from(i);
            bin.1 += f64::from(i);
        }
    }
    bins
}

/// Most intense bin within `half_width` Da of `target`: (bin, intensity).
fn apex_near(bins: &Bins, target: f64, half_width: f64) -> Option<(i64, f64)> {
    let lo = ((target - half_width) / BIN_DA).floor() as i64;
    let hi = ((target + half_width) / BIN_DA).floor() as i64;
    bins.range(lo..=hi)
        .map(|(&k, &(_, i))| (k, i))
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// Intensity-weighted m/z of the bins above half height around `apex`,
/// allowing gaps of a few empty bins so sparse profiles form one peak.
fn centroid(bins: &Bins, apex: i64) -> f64 {
    let half = bins[&apex].1 * 0.5;
    let (mut sum_mi, mut sum_i) = (0.0, 0.0);
    for dir in [-1i64, 1] {
        let mut k = if dir < 0 { apex } else { apex + 1 };
        let mut gap = 0;
        while gap <= 3 {
            match bins.get(&k) {
                Some(&(mi, i)) if i >= half => {
                    sum_mi += mi;
                    sum_i += i;
                    gap = 0;
                }
                Some(_) => break,
                None => gap += 1,
            }
            k += dir;
        }
    }
    sum_mi / sum_i
}

/// Centroid and apex intensity of the reference peak in one scan.
///
/// The most intense peak within 0.5 Da of `reference` counts only when it is
/// at least 10% of the scan base peak and its first 13C isotope appears one
/// isotope spacing (1.00335 / `charge`) higher, within 0.02 Da. Noise near a
/// missing or badly decoded reference is therefore not scored.
fn reference_centroid(
    mz: &[f64],
    intensity: &[f32],
    reference: f64,
    charge: f64,
) -> Option<(f64, f64)> {
    let base = f64::from(intensity.iter().copied().fold(0.0f32, f32::max));
    let bins = bin_points(mz, intensity, reference);
    let (apex, apex_i) = apex_near(&bins, reference, 0.5)?;
    if apex_i < 0.1 * base {
        return None;
    }
    let mono = centroid(&bins, apex);
    let (iso_apex, iso_i) = apex_near(&bins, mono + 1.00335 / charge, 0.02)?;
    if iso_i < 0.1 * apex_i {
        return None;
    }
    let spacing = centroid(&bins, iso_apex) - mono;
    ((spacing - 1.00335 / charge).abs() <= 0.02).then_some((mono, apex_i))
}

enum LockResult {
    Pass(f64, f64),
    Fail(f64, f64),
    NoReference,
}

fn check_lock_function(reader: &Reader, f: &FunctionEntry) -> LockResult {
    let (lo, hi) = (f64::from(f.info.mz_low), f64::from(f.info.mz_high));
    // A declared reference is taken as singly charged (leucine enkephalin in
    // every file seen so far).
    let candidates: Vec<(f64, f64)> = match declared_reference(&reader.dir) {
        Some(reference) => vec![(reference, 1.0)],
        None => LOCK_COMPOUNDS
            .into_iter()
            .filter(|&(m, _)| m > lo && m < hi)
            .collect(),
    };
    let n = f.scan_count();
    let picks: Vec<usize> = (0..LOCK_SCANS.min(n))
        .map(|j| n * (2 * j + 1) / (2 * LOCK_SCANS.min(n)))
        .collect();
    let spectra: Vec<(Vec<f64>, Vec<f32>)> = picks
        .iter()
        .filter_map(|&i| reader.decode_scan(f.index, i).ok())
        .map(|scan| points(scan.spectrum))
        .collect();
    // Score each candidate by summed apex intensity; keep the strongest.
    let mut best: Option<(f64, f64, Vec<f64>)> = None;
    for (reference, charge) in candidates {
        let mut errors = Vec::new();
        let mut signal = 0.0;
        for (mz, intensity) in &spectra {
            if let Some((centroid, apex)) = reference_centroid(mz, intensity, reference, charge) {
                errors.push((centroid - reference) / reference * 1e6);
                signal += apex;
            }
        }
        if !errors.is_empty() && best.as_ref().is_none_or(|b| signal > b.1) {
            best = Some((reference, signal, errors));
        }
    }
    let Some((reference, _, mut errors)) = best else {
        return LockResult::NoReference;
    };
    errors.sort_by(f64::total_cmp);
    let median = errors[errors.len() / 2];
    if median.abs() <= LOCK_TOLERANCE_PPM {
        LockResult::Pass(reference, median)
    } else {
        LockResult::Fail(reference, median)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("expected a corpus directory")?;
    let lock_only = std::env::args().nth(2).as_deref() == Some("--lock-only");
    let paths = bundles(&root)?;
    if paths.is_empty() {
        return Err(format!("no RAW bundles found under {}", root.display()).into());
    }
    let mut complete = 0usize;
    let mut opened_scans = 0usize;
    let mut decoded_scans = 0usize;
    let (mut lock_pass, mut lock_fail, mut lock_noref) = (0usize, 0usize, 0usize);
    for path in &paths {
        match Reader::open(path) {
            Err(error) => println!("OPEN_ERROR {}: {error}", path.display()),
            Ok(reader) => {
                let total = reader.total_scan_count();
                let mut decoded = 0usize;
                let mut nonempty = 0usize;
                let mut first_error = None;
                for result in reader
                    .iter_spectra()
                    .take(if lock_only { 0 } else { total })
                {
                    match result {
                        Ok(scan) => {
                            decoded += 1;
                            nonempty += usize::from(!points(scan.spectrum).0.is_empty());
                        }
                        Err(error) => {
                            first_error.get_or_insert_with(|| error.to_string());
                        }
                    }
                }
                opened_scans += total;
                decoded_scans += decoded;
                if total > 0 && decoded == total {
                    complete += 1;
                }
                if !lock_only {
                    println!(
                        "{} scans={total} decoded={decoded} nonempty={nonempty} first_error={}",
                        path.display(),
                        first_error.as_deref().unwrap_or("")
                    );
                }
                for f in reader.functions.iter().filter(|f| f.info.is_lock_mass()) {
                    let label = format!("LOCK {} function {}", path.display(), f.index);
                    match check_lock_function(&reader, f) {
                        LockResult::Pass(reference, ppm) => {
                            lock_pass += 1;
                            println!("{label} reference={reference} median_ppm={ppm:.1} PASS");
                        }
                        LockResult::Fail(reference, ppm) => {
                            lock_fail += 1;
                            println!("{label} reference={reference} median_ppm={ppm:.1} FAIL");
                        }
                        LockResult::NoReference => {
                            lock_noref += 1;
                            println!("{label} NO_REFERENCE_PEAK");
                        }
                    }
                }
            }
        }
        std::io::stdout().flush()?;
    }
    println!(
        "SUMMARY bundles={} complete={} opened_scans={} decoded_scans={} \
         lock_pass={lock_pass} lock_fail={lock_fail} lock_no_reference={lock_noref}",
        paths.len(),
        complete,
        opened_scans,
        decoded_scans
    );
    if (!lock_only && complete != paths.len()) || lock_fail > 0 {
        std::process::exit(1);
    }
    Ok(())
}
