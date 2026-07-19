# OpenWRaw

[![CI](https://github.com/Sigilweaver/OpenWRaw/actions/workflows/ci.yml/badge.svg)](https://github.com/Sigilweaver/OpenWRaw/actions/workflows/ci.yml)
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.20470607.svg)](https://doi.org/10.5281/zenodo.20470607)
[![crates.io](https://img.shields.io/crates/v/openwraw.svg)](https://crates.io/crates/openwraw)
[![PyPI](https://img.shields.io/pypi/v/openwraw.svg)](https://pypi.org/project/openwraw/)
[![docs.rs](https://img.shields.io/docsrs/openwraw)](https://docs.rs/openwraw)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

> Part of the [OpenMassSpec](https://sigilweaver.app/openmassspec/docs/)
> stack for mass spectrometry raw-file access. Sibling readers:
> [OpenTFRaw](https://github.com/Sigilweaver/OpenTFRaw) (Thermo),
> [OpenTimsTDF](https://github.com/Sigilweaver/OpenTimsTDF) (Bruker).

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

## Repository layout

```
crates/
  openwraw/      Core Rust library (96 tests)
  openwraw-py/   PyO3 / maturin Python bindings
docs/            Docusaurus site (format spec + guides)
```

## License

Apache-2.0. See [LICENSE](LICENSE).

The format specification was developed by binary analysis of public
mass-spectrometry datasets (PRIDE accessions). See
[ATTRIBUTION.md](ATTRIBUTION.md).
