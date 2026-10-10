//! Lock-mass accuracy on 30-byte-index (Variant B) bundles.
//!
//! The lock-mass function of each bundle infuses leucine enkephalin, whose
//! [M+H]+ ion is at m/z 556.2766. Decoded with the right record layout and
//! the `_HEADER.TXT` calibration, its centroid lands within tens of ppm
//! (no lock-mass correction is applied). A wrong record model is hundreds of
//! ppm off or loses the peak and its 13C isotope entirely.
//!
//! Uses public bundles under the corpus root named by `OPENWRAW_CORPUS`.
//! Skips when absent, or fails when `REQUIRE_CORPUS=1` (see
//! `src/test_corpus.rs`).

#[path = "../src/test_corpus.rs"]
mod test_corpus;

use openwraw::{Encoding, Reader};

const LEU_ENK_MH: f64 = 556.2766;
const C13_SPACING: f64 = 1.00335;
const TOLERANCE_PPM: f64 = 100.0;
const SCANS: usize = 20;

/// Intensity-weighted m/z of the points within 0.02 Da of the most intense
/// point within `half_width` Da of `target`, with that point's intensity.
fn centroid_near(
    mz: &[f64],
    intensity: &[f32],
    target: f64,
    half_width: f64,
) -> Option<(f64, f64)> {
    let (apex_mz, apex_i) = mz
        .iter()
        .zip(intensity)
        .filter(|(m, _)| (**m - target).abs() <= half_width)
        .map(|(&m, &i)| (m, f64::from(i)))
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    let (mut sum_mi, mut sum_i) = (0.0, 0.0);
    for (&m, &i) in mz.iter().zip(intensity) {
        if (m - apex_mz).abs() <= 0.02 {
            sum_mi += m * f64::from(i);
            sum_i += f64::from(i);
        }
    }
    Some((sum_mi / sum_i, apex_i))
}

/// Median ppm error of the leucine enkephalin centroid over evenly spaced
/// lock-mass scans, counting only scans where the peak is at least 10% of
/// the base peak and its 13C isotope sits one isotope spacing higher.
fn lock_mass_ppm(reader: &Reader, expected: Encoding) -> f64 {
    let lock = reader
        .functions
        .iter()
        .find(|f| f.info.is_lock_mass())
        .expect("bundle has a lock-mass function");
    assert_eq!(lock.encoding, expected, "function {} encoding", lock.index);
    let n = lock.scan_count();
    let samples = n.min(SCANS);
    let mut errors = Vec::new();
    for j in 0..samples {
        let scan = reader
            .decode_scan(lock.index, n * (2 * j + 1) / (2 * samples))
            .expect("lock-mass scan decodes");
        let (mz, intensity) = (&scan.spectrum.mz, &scan.spectrum.intensity);
        let base = f64::from(intensity.iter().copied().fold(0.0f32, f32::max));
        let Some((mono, apex)) = centroid_near(mz, intensity, LEU_ENK_MH, 0.3) else {
            continue;
        };
        if apex < 0.1 * base {
            continue;
        }
        let Some((iso, _)) = centroid_near(mz, intensity, mono + C13_SPACING, 0.02) else {
            continue;
        };
        if (iso - mono - C13_SPACING).abs() > 0.01 {
            continue;
        }
        errors.push((mono - LEU_ENK_MH) / LEU_ENK_MH * 1e6);
    }
    assert!(
        errors.len() * 2 >= samples,
        "reference peak with 13C isotope found in only {} of {samples} scans",
        errors.len()
    );
    errors.sort_by(f64::total_cmp);
    errors[errors.len() / 2]
}

fn check(candidates: &[&str], expected: Encoding) {
    let Some(dir) = test_corpus::bundle(candidates) else {
        return;
    };
    let reader = Reader::open(&dir).expect("open bundle");
    let ppm = lock_mass_ppm(&reader, expected);
    eprintln!("{}: leucine enkephalin median {ppm:.1} ppm", dir.display());
    assert!(
        ppm.abs() <= TOLERANCE_PPM,
        "{}: median lock-mass error {ppm:.1} ppm exceeds {TOLERANCE_PPM} ppm",
        dir.display()
    );
}

#[test]
fn synapt_g2_si_lock_mass_within_tolerance() {
    check(
        &[
            "PXD068881/20220517_CtpA_1076_2h_1.raw",
            "PXD080129/186_nr15.raw",
        ],
        Encoding::D,
    );
}

#[test]
fn synapt_xs_lock_mass_within_tolerance() {
    check(&["PXD071342/250512_ZMM_MDE_WT2_DIA.raw"], Encoding::D);
}

#[test]
fn xevo_g2_xs_lock_mass_within_tolerance() {
    check(
        &[
            "PXD078353/26-04-24_Set2_MUT_G260D_mapp1.raw",
            "PXD075602/DHPR_11257-1.raw",
        ],
        Encoding::D,
    );
}

/// Some Xevo G2-XS lock-mass functions pair the 30-byte index with 12-byte
/// (Encoding E) records.
#[test]
fn xevo_g2_xs_twelve_byte_lock_mass_within_tolerance() {
    check(
        &[
            "PXD053170/20231113_NSE_Sample_High.raw",
            "PXD045625/Abu_190520_Sha11.raw",
        ],
        Encoding::E,
    );
}
