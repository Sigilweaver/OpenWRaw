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
