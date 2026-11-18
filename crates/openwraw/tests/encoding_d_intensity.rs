//! Encoding D intensity scale on a public bundle (PXD058812).
//!
//! Reads `ITEM_M11_7_His_tag_01.raw` (SYNAPT G2-S, one function, Encoding D)
//! from the corpus root named by `OPENWRAW_CORPUS`; skips when absent (see
//! `src/test_corpus.rs`).

#[path = "../src/test_corpus.rs"]
mod test_corpus;

use openwraw::reader::{Encoding, Reader};

#[test]
fn encoding_d_scan_0_has_a_physical_base_peak() {
    let Some(dir) = test_corpus::bundle(&["PXD058812/ITEM_M11_7_His_tag_01.raw"]) else {
        return;
    };
    let reader = Reader::open(&dir).expect("open bundle");
    assert_eq!(reader.functions[0].encoding, Encoding::D);
    let spec = reader.decode_scan(1, 0).expect("decode scan 0").spectrum;
    assert!(!spec.mz.is_empty());
    let (apex, max) = spec
        .intensity
        .iter()
        .copied()
        .enumerate()
        .fold(
            (0, 0f32),
            |best, (i, v)| if v > best.1 { (i, v) } else { best },
        );
    assert_eq!(max, 30594.0);
    assert!((spec.mz[apex] - 650.97).abs() < 0.05, "{}", spec.mz[apex]);
    // Single-ion noise sits near 28 counts, far below the base peak.
    let min = spec.intensity.iter().copied().fold(f32::MAX, f32::min);
    assert!(min > 0.0 && min < 100.0, "{min}");
}
