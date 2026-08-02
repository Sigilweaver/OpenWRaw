//! Conformance harness: every spectrum produced by the openwraw
//! `collect_records` pipeline must satisfy the invariants in
//! `openmassspec-core`.
//!
//! Looks for a small Waters bundle from PXD058812; skips silently when
//! absent so CI without the corpus is happy.
//!
//! In CI, `.github/workflows/ci.yml`'s `rust` job downloads and unzips
//! the `ITEM_M11_7_His_tag_01.raw` bundle to the workspace root ahead of
//! `cargo test` (Linux and macOS runners only), so this test exercises a
//! real decode path there instead of skipping - see
//! Sigilweaver/OpenWRaw#27.

use std::path::PathBuf;

use openmassspec_core::conformance::assert_iter_invariants;
use openwraw::{mzml::collect_records, Reader};

fn fixture() -> Option<PathBuf> {
    let candidates = [
        // CI / repo-root (gitignored; populated by ci.yml's rust job).
        // This is the one CI actually uses.
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ITEM_M11_7_His_tag_01.raw"),
        // Local dev setups that have a sibling SpecLance checkout with
        // the full corpus available.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../SpecLance/corpus/waters/PXD058812/molecular_mass_P15_01.raw"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../SpecLance/corpus/waters/PXD058812/MS_fragmentation_P29_01.raw"),
    ];
    candidates
        .into_iter()
        .find(|p| p.join("_HEADER.TXT").exists())
}

#[test]
fn openwraw_conformance() {
    let Some(dir) = fixture() else {
        eprintln!("skipping: no Waters bundle available");
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
