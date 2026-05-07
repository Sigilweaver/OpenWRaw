# OpenWRaw

Open-source, cross-platform reader for the Waters MassLynx RAW mass
spectrometry data format.

Waters RAW is the native format produced by Waters LC-MS instruments
(Synapt, Xevo, ACQUITY, and related product lines). No open-source,
dependency-free reader currently exists: all existing tools either
require the proprietary Windows-only MassLynx SDK or wrap it via
ProteoWizard on Windows.

OpenWRaw reverse-engineers the format from a corpus of public datasets
and implements a clean reader in Rust, with Python bindings.

## Status

Early-stage reverse engineering. The format is not yet fully decoded.
See [docs/format/00-overview.md](docs/format/00-overview.md) for
current documentation of the format.

## Repository Structure

```
corpus/         Sample .raw directories from PRIDE (gitignored, on ZFS)
analysis/       Python/uv tooling for corpus acquisition and binary analysis
crates/
  openwraw/     Core Rust library
  openwraw-cli/ CLI tool (inspect, convert to mzML)
docs/
  format/       Reverse-engineered format documentation
re/             Working notes (gitignored)
```

## Building

Requires: Rust 1.95+ (stable), cargo

```
cargo build --workspace
```

## Analysis Tooling

Requires: Python 3.12+, uv

```
cd analysis
uv sync
```

## License

Apache-2.0
