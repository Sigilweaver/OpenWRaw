# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-05-16

### Added

- Pure-Rust reader for the Waters MassLynx RAW directory format with
  zero external dependencies in the core crate.
- Three spectrum encodings decoded and validated against PRIDE corpora:
  - **Encoding A**: 6-byte records (QTOF Ultima class).
  - **Encoding B**: 8-byte IMS cells (SYNAPT G2-Si with drift time).
  - **Encoding C**: 8-byte sub-bin (Xevo G2-XS QTof).
- Two scan-index variants decoded:
  - **Variant A**: 22-byte records.
  - **Variant B**: 30-byte records (modern instruments).
- Parsers for ancillary files: `_HEADER.TXT`, `_extern.inf`,
  `_FUNCTNS.INF`, `_FUNCnnn.IDX`, `_FUNCnnn.DAT`, `_CHROMS.INF`,
  `_CHROnnnn.DAT`.
- `openwraw-cli` with `inspect` and `convert` (mzML) subcommands.
- `openwraw-py` PyO3 Python bindings (`RawReader`, `read_spectrum`,
  `read_ims_spectrum`, `read_chrom`).
- 69 unit and integration tests covering the core crate.
- Format specification under `docs/format/` (11 numbered documents
  covering each on-disk file).

### Out of scope

- Vendor-DLL paths or any Windows-only dependency.
- Function types beyond MS / MS/MS / chromatographic channels.

[0.1.0]: https://github.com/Sigilweaver/OpenWRaw/releases/tag/v0.1.0
