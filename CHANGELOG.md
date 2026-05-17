# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.1] - 2026-05-17

### Fixed

- `pyproject.toml`: add `[tool.maturin] include` directive so the sdist
  contains `LICENSE`, `README.md`, and `CHANGELOG.md`; PyPI was rejecting
  the 1.0.0 sdist because `PKG-INFO` declared `License-File: LICENSE` but
  the file was absent from the tarball.

### Changed

- README: standardize docs link format and section structure.
- Docs: replace prose `--` with `-` in three pages (`guide/reader.md`,
  `format/03-func-idx.md`, `format/04-func-dat.md`).

## [1.0.0] - 2026-05-17

First stable release. The public API of `openwraw` is now considered
stable and will follow semantic versioning. Format coverage is unchanged
from 0.1.0 (QTOF Ultima, SYNAPT G2-Si, Xevo G2-XS QTof).

### Added

- `ATTRIBUTION.md`: tracks third-party notices for bundled data.
- `publish.yml` GitHub Actions workflow: publishes the `openwraw` crate
  to crates.io and the Python wheel to PyPI via OIDC Trusted Publishing
  on every `v*` tag push.
- `Cargo.lock` is now committed to the repository for reproducible builds.
- `[project.urls]` added to `pyproject.toml` (Homepage, Documentation,
  Repository, Changelog).

### Changed

- Removed `openwraw-cli` crate: the CLI added unnecessary complexity for
  a library-focused project; the Python bindings (`openwraw-py`) cover
  interactive exploration needs adequately.
- CI migrated from WarpBuild runners to standard GitHub-hosted
  (`ubuntu-latest`, `macos-latest`, `windows-latest`).

## [0.1.0] - 2026-05-16

### Added

- Rust reader for the Waters MassLynx RAW directory format with
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
- `openwraw-py` PyO3 Python bindings (`RawReader`, `read_spectrum`,
  `read_ims_spectrum`, `read_chrom`).
- 69 unit and integration tests covering the core crate.
- Format specification under `docs/format/` (11 numbered documents
  covering each on-disk file).

### Out of scope

- Function types beyond MS / MS/MS / chromatographic channels.

[1.0.1]: https://github.com/Sigilweaver/OpenWRaw/releases/tag/v1.0.1
[1.0.0]: https://github.com/Sigilweaver/OpenWRaw/releases/tag/v1.0.0
[0.1.0]: https://github.com/Sigilweaver/OpenWRaw/releases/tag/v0.1.0
