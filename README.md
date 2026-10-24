# OpenWRaw

[![CI](https://github.com/Sigilweaver/OpenWRaw/actions/workflows/ci.yml/badge.svg)](https://github.com/Sigilweaver/OpenWRaw/actions/workflows/ci.yml)
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.20470607.svg)](https://doi.org/10.5281/zenodo.20470607)
[![crates.io](https://img.shields.io/crates/v/openwraw.svg)](https://crates.io/crates/openwraw)
[![PyPI](https://img.shields.io/pypi/v/openwraw.svg)](https://pypi.org/project/openwraw/)
[![docs.rs](https://img.shields.io/docsrs/openwraw)](https://docs.rs/openwraw)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/Sigilweaver/OpenWRaw/blob/main/LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)

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

## Known issues and gaps

- **m/z is not lock-mass corrected, and is typically 30 to 100 ppm off.**
  m/z uses only the calibration stored in `_HEADER.TXT`. Lock-mass
  (reference) functions are skipped when iterating spectra and are not used
  to correct the others. Measured on the 69 lock-mass functions of the
  public test corpus, the reference ion reads about 46 ppm from its true
  m/z (median), 98 ppm at the 95th percentile and up to 162 ppm. For
  accurate-mass work, recalibrate
  against known masses (most search engines have a mass recalibration
  step; the bundle's own lock-mass scans are available through
  `Reader::decode_scan`), or search with tolerances of at least 100 ppm.
- **No ion mobility decoding.** SYNAPT, Vion and Cyclic IMS scans are
  returned as m/z and intensity only. There is no drift-time output, and
  mzML output carries no mobility arrays.
- **DDA precursors carry m/z only.** Data-dependent MS/MS scans report the
  precursor m/z recorded for each scan, as both isolation window target and
  selected ion m/z. No charge state, isolation width or reference to the
  triggering survey scan is reported; none has been found in the files.
- **MS level of Vion MSe data.** The high-energy function of an MSe
  acquisition is reported as MS2 when the bundle labels it `TOF PARENT`, or
  labels it `TOF MS` and records its collision-energy ramp. Vion bundles
  exported by UNIFI record no collision energy, so both of their `TOF MS`
  functions are reported as MS1.
- **Polarity is reported only when the bundle records it.** UNIFI exports
  do not, so their spectra have no polarity.
- **Encoding E** (12-byte records): intensity flag bits and the auxiliary
  word are not interpreted; flagged peaks are kept. Encoding E spectra are
  labelled centroid. That is confirmed from point spacing for the Xevo
  G2-XS lock-mass functions but not for LCT Premier bundles.
- **Tested instruments.** The public test corpus covers Q-Tof, SYNAPT, Vion,
  Cyclic IMS and Xevo G2-XS/G3 QTof data in positive mode, including MSe,
  HDMSe, DDA, targeted MS/MS and one MALDI MS/MS bundle. Triple and single
  quadrupole (MRM/SIR), GC and negative-mode data are untested.

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

The [2026-09-26 corpus audit](https://github.com/Sigilweaver/OpenWRaw/blob/main/CORPUS-AUDIT.md) records the expanded
corpus results and the format gaps it exposed.

## Repository layout

```
crates/
  openwraw/      Core Rust library
  openwraw-py/   PyO3 / maturin Python bindings
docs/            Docusaurus site (format spec + guides)
```

## License

Apache-2.0. See [LICENSE](https://github.com/Sigilweaver/OpenWRaw/blob/main/LICENSE).

The format specification was developed by binary analysis of public
mass-spectrometry datasets (PRIDE accessions). See
[ATTRIBUTION.md](https://github.com/Sigilweaver/OpenWRaw/blob/main/ATTRIBUTION.md).
