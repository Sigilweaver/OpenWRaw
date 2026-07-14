# Fuzzing

`fuzz_reader` fuzzes the reader entry point every caller (Rust and Python)
goes through: `Reader::open` plus decoding every scan via
`Reader::iter_spectra`. The harness splits the fuzzer's byte string into
five length-prefixed chunks and writes them out as `_HEADER.TXT`,
`_extern.inf`, `_FUNCTNS.INF`, `_FUNC001.IDX`, and `_FUNC001.DAT` in a temp
directory, so a single input can exercise the whole decode pipeline
(metadata parsing, scan index, and the Encoding A/B/C decoders) end to end.

## Running

```sh
cargo install cargo-fuzz
cargo +nightly fuzz run fuzz_reader fuzz/seed_corpus
```

`fuzz/corpus/` (the fuzzer's working corpus, gitignored) is separate from
`fuzz/seed_corpus/` (tracked). Point a real fuzzing session at the tracked
seeds so it starts from inputs that already parse past the ASCII keyword
gates in `_extern.inf` (`Lteff`, `Veff`, `PusherInterval`), rather than
relying on random mutation to discover those strings from scratch:

```sh
mkdir -p fuzz/corpus/fuzz_reader
cp fuzz/seed_corpus/* fuzz/corpus/fuzz_reader/
cargo +nightly fuzz run fuzz_reader
```

## Seeds

- `seed_pxd058812`: built from the small public Waters bundle from PRIDE
  PXD058812 (the same one CI downloads for `validate-mzml` and the Python
  test suite), truncated for corpus size. Gives the fuzzer a
  fully-realistic starting point.
- `seed_minimal`: a small hand-built bundle with normal, in-bounds scan
  offsets. Fast to mutate.
- `seed_huge_offset_regression`: locks in the fix for a `.IDX` scan whose
  Variant B `dat_offset` fields, taken at face value, imply a scan length
  of ~4.29 GB while the real `.DAT` file is 64 bytes. Before `scan_slice`
  capped the computed length against the real, already-known `.DAT` file
  size, this made `read_slice` attempt an allocation sized from unvalidated
  file-controlled offsets - under a virtual-memory limit (a realistic
  hardening measure), that aborts the process (`SIGABRT`) rather than
  returning a `Result::Err`.
