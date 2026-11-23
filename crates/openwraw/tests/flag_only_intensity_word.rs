//! Encoding D flag-only intensity word on a public bundle (PXD053170).
//!
//! Reads `20231113_NSE_Sample_High.raw` (Xevo G2-XS, 2.8 GB function 1, too
//! large for CI) from the corpus root named by `OPENWRAW_CORPUS`; skips when
//! absent (see `src/test_corpus.rs`).

#[path = "../src/test_corpus.rs"]
mod test_corpus;

use openwraw::reader::{Encoding, Reader};

/// Function 1 scan 42 holds one intensity word `0x20000000` (flag bit 29, no
/// mantissa). It decodes to a kept point with intensity 0.0 in front of a
/// flagged, nonzero point, and the scan decodes without error.
#[test]
fn flag_only_intensity_word_decodes_to_a_zero_point() {
    let Some(dir) = test_corpus::bundle(&["PXD053170/20231113_NSE_Sample_High.raw"]) else {
        return;
    };
    let reader = Reader::open(&dir).expect("open bundle");
    assert_eq!(reader.functions[0].encoding, Encoding::D);
    let spec = reader.decode_scan(1, 42).expect("decode scan 42").spectrum;
    let zeros: Vec<usize> = (0..spec.intensity.len())
        .filter(|&i| spec.intensity[i] == 0.0)
        .collect();
    assert_eq!(zeros.len(), 1, "one flagged zero point");
    let i = zeros[0];
    assert!((spec.mz[i] - 797.51).abs() < 0.05, "{}", spec.mz[i]);
    assert!(spec.intensity[i + 1] > 0.0);
    assert!(spec.mz[i] > spec.mz[i - 1] && spec.mz[i] < spec.mz[i + 1]);
}
