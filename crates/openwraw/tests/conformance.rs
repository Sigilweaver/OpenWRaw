//! Conformance harness: every spectrum produced by the openwraw
//! `collect_records` pipeline must satisfy the invariants in
//! `openmassspec-core`.
//!
//! Uses a small public Waters bundle from PXD058812 under the corpus root
//! named by `OPENWRAW_CORPUS`. Skips when absent, or fails when
//! `REQUIRE_CORPUS=1` (see `src/test_corpus.rs`).
//!
//! In CI, `.github/workflows/ci.yml`'s `rust` job downloads the
//! `ITEM_M11_7_His_tag_01.raw` bundle into `corpus/PXD058812/` and runs this
//! test with `REQUIRE_CORPUS=1` (Linux and macOS runners only), so it
//! exercises a real decode path there instead of skipping - see
//! Sigilweaver/OpenWRaw#27.

#[path = "../src/test_corpus.rs"]
mod test_corpus;

use openmassspec_core::conformance::assert_iter_invariants;
use openwraw::{mzml::collect_records, Reader};

#[test]
fn openwraw_conformance() {
    let Some(dir) = test_corpus::bundle(&[
        // The bundle CI downloads.
        "PXD058812/ITEM_M11_7_His_tag_01.raw",
        // Other small bundles from the same public dataset.
        "PXD058812/molecular_mass_P15_01.raw",
        "PXD058812/MS_fragmentation_P29_01.raw",
    ]) else {
        return;
    };
    let reader = Reader::open(&dir).expect("open bundle");
    let records = collect_records(&reader).expect("collect records");
    let total = records.len();
    let n = assert_iter_invariants(records).expect("conformance");
    assert_eq!(n, total);
    assert!(
        n > 0,
        "expected at least one spectrum from {}",
        dir.display()
    );
    eprintln!("openwraw: {n} spectra passed conformance");
}
