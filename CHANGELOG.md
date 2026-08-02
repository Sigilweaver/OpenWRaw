# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- CI's `rust` job now downloads and unzips the same small PRIDE
  PXD058812 Waters bundle the `python` and `validate-mzml` jobs already
  use (Linux and macOS runners), ahead of `cargo test`, so
  `crates/openwraw/tests/conformance.rs` exercises a real decode path
  in CI instead of always skipping (Sigilweaver/OpenWRaw#27).

## [1.2.8] - 2026-07-29

### Fixed

- Adapted `RunMetadata` construction to `openmassspec-core` 1.4.0's new
  `acquisition_software_name`/`acquisition_software_version` fields
  (defaulted to `None`; neither `_extern.inf` nor `_HEADER.TXT` parsing
  in this reader currently exposes a MassLynx acquisition-software
  version string as a struct field - `_HEADER.TXT`'s `Version` field is
  documented as the MassLynx *file format* version, a different concept
  - so wiring in a real value would mean adding new parsing, which is
  out of scope here). Bumped the declared `openmassspec-core` minimum
  to `"1.4.0"` to match (Sigilweaver/OpenWRaw#25).

## [1.2.7] - 2026-07-25

### Fixed

- Adapted `RunMetadata` construction to `openmassspec-core` 1.3.0's new
  `analyzers`/`instrument_serial_number` fields (defaulted, as neither is
  decoded here; `PrecursorInfo::ccs` already defaulted to `None` via
  this reader's `..Default::default()` precursor construction - CCS
  derivation from drift time is tracked separately, #10).
- Declared `openmassspec-core` minimum was still `"1.0.0"`; now that the
  code needs 1.3.0's new fields to compile, bumped the declared minimum
  to `"1.3.0"` to match.

## [1.2.6] - 2026-07-20

### Added

- Precursor metadata (Sigilweaver/OpenWRaw#8, #13): `SpectrumRecord::precursor`
  is no longer hardcoded `None` for every spectrum. `target_mz` is now parsed
  from `_extern.inf`'s `Set Mass` field on real targeted MS/MS functions
  (`TOF MSMS FUNCTION` / `TOF DAUGHTER FUNCTION`), confirmed against a new
  corpus sample (PXD035818) found specifically to unblock this - the
  existing corpus was exclusively broadband MSe/HDMSe
  (`Precursor Selection: Everything`), which genuinely has no discrete
  precursor and correctly keeps `target_mz: None`. Separately,
  `collision_energy` is now read from `_FUNCnnn.STS`'s per-scan "Collision
  Energy" channel (new `raw::func_sts` module) for every MS2 function,
  including MSe - Waters records a real collision energy for the
  high-energy MSe scan even without a discrete precursor. No charge state
  or isolation width field has been found in either source file on any
  corpus sample; those remain `None`.
- `WatersSource::iter_chromatograms` (Sigilweaver/OpenWRaw#9): decodes
  `_CHROMS.INF`/`_CHROnnnn.DAT` instrument channels (pump pressure, flow
  rate, temperature) into `openmassspec_core::ChromatogramRecord`. Only
  channels whose units map to a real PSI-MS chromatogram-type term
  (pressure/flow-rate/temperature, verified against psi-ms.obo) are
  emitted; channels with no CV match (e.g. solvent composition %, heater
  power %) are skipped rather than mislabeled or defaulted to "total ion
  current chromatogram".

## [1.2.5] - 2026-07-15

### Fixed

- `instrument_cv`'s lookup table carried mostly-wrong PSI-MS accessions,
  checked directly against psi-ms.obo rather than trusting the existing
  table: of 11 entries, only `XEVO TQ-S` and the generic fallback pointed
  at the right term. Some were wrong by a wide margin - `XEVO G2-XS QTOF`
  resolved to `MS:1002472` ("trap-type collision-induced dissociation"),
  bare `XEVO` resolved to `MS:1000533` ("Bioworks", unrelated Thermo
  software). Fixed every unambiguous entry; dropped `SYNAPT G2-S`/
  `SYNAPT G2`/bare `SYNAPT` rather than guess between the CV's separate
  HDMS/MS variants for each (tracked in #11) - they fall through to the
  generic Waters term instead of asserting a wrong specific one.
- `start_timestamp` was built as `"{date} {time}"` from Waters' native
  `"14-Jan-2021"`/`"16:20:52"` header strings - nowhere close to RFC
  3339. Harmless while the shared writer silently dropped
  `start_timestamp`; `openmassspec-core` 1.2.0 now emits it as an
  `xs:dateTime` XML attribute, so this would have produced invalid mzML
  the first time this crate's dependency was bumped. Reformats into
  `YYYY-MM-DDTHH:MM:SSZ` (same trailing-Z-for-a-timezone-less-local-time
  convention `opentfraw` uses for the identical problem).
- Bumped `openmassspec-core` to 1.2.0 and added the `SpectrumRecord.faims_cv`
  field it requires, fixing a build break: 1.2.0 added that field as
  required, and `record_from_scan` constructed the struct literal without
  it. Always `None` - Waters instruments have no FAIMS interface.

## [1.2.4] - 2026-07-14

### Added

- Python-side `tests/` directory with a pytest suite exercising every
  `RawReader` method and return type (`RunHeader`, `FunctionInfo`,
  `ChromChannel`, `ChromPoint`, `Spectrum`, `ImsSpectrum`) against a real
  Waters bundle, asserting on shape/type rather than exact values per the
  clean-room policy. The `raw_bundle` fixture downloads and caches the same
  `$WATERS_ZIP_URL` bundle `ci.yml`'s `validate-mzml` job already uses. CI's
  `python` job now installs `pytest`, downloads the bundle, and runs the
  suite after `maturin develop`. Fixes #1. (@Nabejo)
- `fuzz/`: a `cargo fuzz` target (`fuzz_reader`) over the reader entry
  point (`Reader::open` + `Reader::iter_spectra`), exercising the full
  decode pipeline - metadata parsing, scan index, and the Encoding A/B/C
  decoders - from a single fuzzer-supplied byte string. Seeded from the
  same public PXD058812 bundle used elsewhere in CI, a minimal synthetic
  bundle, and a regression input locking in the allocation-cap fix above.
  CI now builds the fuzz target and smoke-runs it against the seed corpus
  on every PR. Part of #3. (@Nabejo)

### Fixed

- CI (`ci.yml`): the `python` job now also runs on `windows-latest`, so the
  Windows wheel `publish.yml` ships is actually built and imported before a
  release. The job's venv paths are OS-conditional (`.venv\Scripts\` on
  Windows, `.venv/bin/` elsewhere) and steps now invoke `python -m pip` /
  `python -m maturin` instead of calling the venv's executables directly.
  Fixes #2. (@Nabejo)
- `Reader::decode_scan`: a Variant B `_FUNCnnn.IDX` scan's length was
  computed as the difference between two raw file-controlled `u32`
  `dat_offset` fields, with no check against the real, already-known size
  of the paired `_FUNCnnn.DAT` file before allocating a buffer of that
  length. A `.IDX` file of a few dozen bytes could claim a scan length of
  up to ~4.29 GB while the real `.DAT` file was a few bytes long; under a
  virtual-memory limit (a realistic hardening measure for processes
  parsing untrusted input) this aborted the process (`SIGABRT`) rather than
  returning a `Result::Err`. `scan_slice` now caps the computed length
  against the real `.DAT` file size for both Variant A and Variant B scans.
  Fixes #3. (@Nabejo)

## [1.2.3] - 2026-07-13

### Fixed

- `WatersSource::iter_spectra` decoded every scan into a `Vec` up front and
  then cloned that `Vec` again, holding two full copies of the run in
  memory before yielding a single spectrum. It now streams scans lazily
  through `Reader::iter_spectra()` directly, skipping scans that fail to
  decode instead of aborting the whole run. Fixes #4.

## [1.2.2] - 2026-07-10

### Changed

- Dependency renamed `openproteo-core` -> `openmassspec-core` (1.0.0),
  following the umbrella's rename from OpenProteo to OpenMassSpec.
  No behavioral change.
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
