# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- `pyo3` bumped from 0.28.3 to 0.29.0, clearing two RustSec advisories
  (RUSTSEC-2026-0177, RUSTSEC-2026-0176).
- `openwraw-py` no longer opts out of the workspace's `unsafe_code =
  "forbid"` lint; it never contained an `unsafe` block, so this was a stale
  exception out of sync with CONTRIBUTING.md's description of the policy.
  Also wires up the `clippy::unwrap_used`/`expect_used` warn lint the core
  crate already carries.
- CI (`ci.yml`) now runs `cargo clippy`/`cargo test` against the full
  workspace (previously `--exclude openwraw-py` on both) and adds
  `windows-latest` to the `rust` job's matrix.
- New `audit.yml` workflow runs `cargo audit` against the RustSec Advisory
  DB on dependency changes and weekly.

### Fixed

- `docs/guide/reader.md` and `docs/guide/chromatograms.md`: the Rust
  examples referenced a `RawReader` type and `.channels()`/`.read_chrom()`
  methods that don't exist on the Rust API (that name belongs only to the
  Python bindings; the Rust type is `Reader`, and chromatogram channels are
  read via free functions in the `chroms` module, not through `Reader`).
  Both examples now compile.

## [1.2.1] - 2026-07-06

### Changed

- PyPI package now declares `keywords` (`mass-spectrometry`, `waters`,
  `masslynx`, `raw`, `proteomics`) so the package is findable via PyPI
  search; previously only the crates.io side had them.

## [1.2.0] - 2026-07-02

### Added

- `RunHeader` class (Python): exposes `_HEADER.TXT` metadata -
  `instrument`, `acquired_date`, `acquired_time`, `operator`,
  `sample_description`, `version`, `acquired_name`. Returned by
  `RawReader.header`.
- `RawReader.polarity` (Python): electrospray polarity from
  `_extern.inf` as `"positive"`, `"negative"`, or `None`.
- `RawReader.ms_level(func_index)` (Python): returns `1` for MS1
  survey and reference functions, `2` for MSe / DDA / targeted MS2
  functions. Falls back to `1` for functions absent from `_extern.inf`.
- `RawReader.function_encoding(func_index)` (Python): returns `"a"`
  for standard Q-TOF (non-IMS) functions or `"b"` for SYNAPT IMS
  functions, indicating which read method to use.

### Changed

- `publish.yml`: crates.io publish step uses `continue-on-error: true`
  so re-triggered tag runs do not fail the workflow when the crate
  version was already published.

## [1.1.0] - 2026-05-31

### Added

- `CITATION.cff`: author identity (Nathan Riley + ORCID) and a
  scaffolded `identifiers:` block ready for the Zenodo concept DOI.
- `CONTRIBUTING.md`.
- Docusaurus build job in CI.

### Changed

- **Panic surface eliminated (WP17).** Parsers no longer call
  `unwrap()` in production code: a new `bytes` helper module
  (`read_u16/u32/f32_le`) returns `Error::Parse` with byte offset.
  Library crate carries `#![cfg_attr(not(test), warn(clippy::
  unwrap_used, clippy::expect_used))]`.
- Manifest hygiene (WP13): `homepage` set to <https://sigilweaver.app>
  and `documentation` link added.
- README badge block unified across the Sigilweaver portfolio.

## [1.0.5] - 2026-05-21

### Changed

- Depend on `openproteo-core = "1.0.0"` (was `0.1.0`, yanked).
- MSRV bumped from 1.75 to 1.85 (tracks `openproteo-core 1.0.0`).

## [1.0.4] - 2026-05-18

### Changed

- Depend on `openproteo-core = "0.1.0"` from crates.io (workspace
  dependency now carries an explicit registry version so the crate can
  be published).
- `SECURITY.md` added; coordinated-disclosure contact documented.

## [1.0.3] - 2026-05-17

### Changed

- Docs and `.gitignore`: replace em-dashes and en-dashes with ASCII
  hyphens for consistent rendering across editors and terminals.

## [1.0.2] - 2026-05-17

### Fixed

- `pyproject.toml`: add `readme = "README.md"` so PyPI renders the long
  description. Include `README.md` in the wheel distribution as well as
  the sdist.

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

[1.0.2]: https://github.com/Sigilweaver/OpenWRaw/releases/tag/v1.0.2
[1.0.1]: https://github.com/Sigilweaver/OpenWRaw/releases/tag/v1.0.1
[1.0.0]: https://github.com/Sigilweaver/OpenWRaw/releases/tag/v1.0.0
[0.1.0]: https://github.com/Sigilweaver/OpenWRaw/releases/tag/v0.1.0
