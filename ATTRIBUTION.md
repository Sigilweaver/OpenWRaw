# Attribution

## Prior art and references

OpenWRaw was developed by binary analysis of public Waters MassLynx `.raw`
datasets. We are grateful to the projects below for prior reverse-engineering
work, open documentation, and tooling that informed this implementation.

- **mzR / ProteoWizard** - earlier open-source readers for vendor mass
  spectrometry formats, which set the precedent for community parsers.
- **The PSI mzML specification** - drove our conversion target format.
- **HDF Group / netCDF tooling** - referenced for chromatogram data models.

OpenWRaw is an independent implementation. It does not include or link to any
Waters proprietary code, libraries, or SDKs. "Waters" and "MassLynx" are
trademarks of Waters Corporation; their use in this project is descriptive
only and does not imply endorsement.

## Validation corpus (PRIDE)

The format specification and reader were validated against public datasets
from the EBI PRIDE Archive. Raw files are not redistributed through this
repository; corpus contents are stored separately. Each dataset retains its
original licence (PRIDE's default is CC-BY 4.0; per-dataset terms always win).

| Accession | Instrument | Notes |
|---|---|---|
| [PXD058812](https://www.ebi.ac.uk/pride/archive/projects/PXD058812) | Q-TOF Ultima | Older MassLynx format; reference for `_extern.inf` `Lteff`/`Veff` parsing |
| [PXD068881](https://www.ebi.ac.uk/pride/archive/projects/PXD068881) | Synapt G2-Si IMS | Reference for `PusherInterval = 69.0`, multi-channel chromatograms |
| [PXD075602](https://www.ebi.ac.uk/pride/archive/projects/PXD075602) | Xevo G2-XS QTof | Newer format; reference for per-function `PusherInterval` overrides |

If you use this validation work, please cite the original PRIDE submitters and
the relevant accession.

## LCT Premier format evidence (MetaboLights)

The 12-byte Encoding E layout was derived from original public acquisitions
in [MTBLS701](https://www.ebi.ac.uk/metabolights/MTBLS701) and
[MTBLS13770](https://www.ebi.ac.uk/metabolights/MTBLS13770). Validation uses
record widths, normalized word patterns, ordered positions, and same-file
scan-index intensity totals. It does not use vendor software or
vendor-derived expected output. Intensity flags and the auxiliary word
remain uninterpreted.

Raw files are kept outside version control and are not redistributed.
MTBLS701 exposes EMBL-EBI Terms of Use; MTBLS13770 explicitly exposes CC0 1.0.
Per-study terms apply. Please cite the original studies when using the data.

## Chromatogram descriptor evidence (issue #36)

The corrected `_CHROMS.INF` layout was derived from native public side-file
bytes, without vendor tools, SDKs, converted spectra, or contributor uploads.
Files were fetched on 2026-10-04 and kept outside tracked source.

| Source | Native bundle | Metadata size | SHA-256 of `_CHROMS.INF` |
|--------|---------------|---------------|--------------------------|
| [PSU Data Commons](https://www.datacommons.psu.edu/download/metabolomics/WAS/kt130808_WAS_0179.raw/) | kt130808_WAS_0179.raw, SYNAPT G2-S | 213 | `8c1327a558139626a59e68b2594e67e66a5f92d8c4792e31de983a241d1a8287` |
| [PSU Data Commons](https://www.datacommons.psu.edu/download/metabolomics/limin/ZLM130522_tcdf_urine_309.raw/) | ZLM130522_tcdf_urine_309.raw, SYNAPT G2-S | 213 | `8c1327a558139626a59e68b2594e67e66a5f92d8c4792e31de983a241d1a8287` |
| [MassIVE MSV000083877](https://massive.ucsd.edu/ProteoSAFe/dataset.jsp?task=a66ade995ac8431e80f6e27f11c55674) | ROF_181101_04_IgG.raw, Xevo G2 QTof | 213 | `e656a9f4616552fea6ada19b13e0d45b15d82f29211ae4b2e088d24a9e11093d` |

MassIVE lists MSV000083877 under CC0 1.0. PSU download listings do not
establish a redistribution license; their raw files are not redistributed.
These files reproduce the reported size and parser error, but are not LCT
Premier XE acquisitions and do not prove the reporter's exact file layout.

The public MTBLS701 LCT Premier acquisition
`1506_SZ_SZ_E01_neg.raw` independently confirms four channel records after
the same descriptor header. PXD068881
`20220517_CtpA_1076_2h_1.raw` confirms seven channels. The preamble values
are `[128, 1, 85, 2]`; the final value counts 48-byte Flags/Description
field descriptors within the header, not 85-byte records after it.
Corresponding native CHRO time series were checked for finite values,
ordered retention times and one-to-one channel numbering.

## Third-party Rust dependencies

The OpenWRaw core (`openwraw`) crate has no
third-party runtime dependencies. The Python bindings crate (`openwraw-py`)
adds:

- `pyo3` (Apache-2.0 OR MIT) - Python interoperability, with its transitive
  build-time crates (`pyo3-build-config`, `pyo3-ffi`, `pyo3-macros`,
  `pyo3-macros-backend`, `proc-macro2`, `quote`, `syn`, `heck`, `libc`,
  `once_cell`, `portable-atomic`, `target-lexicon`, `unicode-ident`).
- `maturin` (build-time, Apache-2.0 OR MIT) - Python wheel build backend.

A full machine-readable list lives in `Cargo.lock`.

## Licence

OpenWRaw itself is released under the Apache License, Version 2.0.
See [LICENSE](LICENSE) for the full text.
