# OpenWRaw

[![CI](https://github.com/Sigilweaver/OpenWRaw/actions/workflows/ci.yml/badge.svg)](https://github.com/Sigilweaver/OpenWRaw/actions/workflows/ci.yml)
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.20470607.svg)](https://doi.org/10.5281/zenodo.20470607)
[![crates.io](https://img.shields.io/crates/v/openwraw.svg)](https://crates.io/crates/openwraw)
[![PyPI](https://img.shields.io/pypi/v/openwraw.svg)](https://pypi.org/project/openwraw/)
[![docs.rs](https://img.shields.io/docsrs/openwraw)](https://docs.rs/openwraw)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

> Part of the [OpenMassSpec](https://github.com/Sigilweaver/OpenMassSpec)
> stack for mass spectrometry raw-file access.

Rust and Python reader for the Waters MassLynx RAW mass spectrometry
data format. Cross-platform (Linux, macOS, Windows), with no native or
system dependencies.

Documentation: [sigilweaver.app/openwraw/docs](https://sigilweaver.app/openwraw/docs)

## Install

**Prefer [`openmassspec-io`](https://github.com/Sigilweaver/OpenMassSpec)
with the `waters` feature/extra** unless you need this parser standalone
(minimal dependencies, or building your own abstraction) - the umbrella
gives you format auto-detection, mzML conversion, and Arrow streaming
across all wired-in vendors for free:

```sh
cargo add openmassspec-io --features waters
```

```sh
pip install openmassspec[waters]
```

Standalone:

Rust:

```sh
cargo add openwraw
```

Python:

```sh
pip install openwraw
```

## Quickstart

Rust:

```rust
use openwraw::Reader;

let r = Reader::open("sample.raw")?;
for f in &r.functions {
    println!("function {}: {} scans", f.index, f.scan_count());
}
```

Python:

```python
import openwraw

r = openwraw.RawReader("sample.raw")
spec = r.read_spectrum(1, 0)
print(spec.mz[:5], spec.intensity[:5])
```

See the [docs site](https://sigilweaver.app/openwraw/docs) for the full
quickstart, guide, and format specification.

## Known issues

- **No ion mobility decoding.** SYNAPT scans are returned as m/z and
  intensity only. There is no drift-time output, and mzML output carries no
  mobility arrays.
- **No lock-mass correction.** m/z uses only the calibration stored in
  `_HEADER.TXT`. Lock-mass functions are skipped when iterating spectra and
  are not used to correct the others. Across the public test corpus the
  uncorrected lock-mass reference sits within about 75 ppm for most
  bundles and up to about 160 ppm for a few.
- Encoding E (12-byte records) intensity flag bits and auxiliary word are
  not interpreted; flagged peaks are kept.

## Debug logging

OpenWRaw logs how it reads a bundle: which side files it resolved
(including sample-prefixed names), instrument and geometry from
`_extern.inf`, and for each function the scan count, mass range, chosen
record encoding and why, and calibration. Warnings flag skipped functions,
missing calibration, unreadable `_FUNCnnn.STS` files, and indexes that
address more bytes than their DAT file holds. Per-scan byte ranges are
logged at trace level.

Rust uses the [`log`](https://docs.rs/log) facade; install any logger:

```sh
RUST_LOG=openwraw=debug your-program sample.raw   # with env_logger
```

Python forwards to the standard `logging` module under the `openwraw`
logger. Configure it before opening files:

```python
import logging
logging.basicConfig(level=logging.DEBUG)

import openwraw
openwraw.RawReader("sample.raw")
```

Include this output when reporting a file that fails to open or decode.

## Corpus decode audit

Run every non-lock-mass scan in a directory tree of Waters RAW bundles:

```sh
cargo run -p openwraw --release --example audit_corpus -- /path/to/corpus
```

The command reports opened bundles, decoded scans, and the first error per
bundle, then checks each lock-mass function against its reference compound
(`ReferenceMass1` in `_extern.inf`, or Leu-Enk / Glu-fib). It exits with an
error if any bundle fails to open, any scan fails to decode, or any lock-mass
function is more than 100 ppm off. Add `--lock-only` to skip the full scan
pass.

The [2026-09-26 corpus audit](CORPUS-AUDIT.md) records the expanded
corpus results and the format gaps it exposed.

## Repository layout

```
crates/
  openwraw/      Core Rust library
  openwraw-py/   PyO3 / maturin Python bindings
docs/            Docusaurus site (format spec + guides)
```

## License

Apache-2.0. See [LICENSE](LICENSE).

The format specification was developed by binary analysis of public
mass-spectrometry datasets (PRIDE accessions). See
[ATTRIBUTION.md](ATTRIBUTION.md).
