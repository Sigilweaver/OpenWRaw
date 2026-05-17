---
sidebar_position: 98
---

# Changelog

The canonical changelog lives at
[`CHANGELOG.md`](https://github.com/Sigilweaver/OpenWRaw/blob/main/CHANGELOG.md)
in the repository root. The notes below mirror the latest release.

## 1.0.0

First stable release. The public API of `openwraw` is now considered
stable and will follow semantic versioning. Format coverage and parser
set are unchanged from 0.1.0.

## 0.1.0

Initial release.

- Rust workspace with `openwraw` (library) and
  `openwraw-py` (Python bindings via PyO3 + maturin).
- Parsers for `_HEADER.TXT`, `_extern.inf`, `_FUNCTNS.INF`,
  `_FUNCnnn.IDX` (Variants A and B), `_FUNCnnn.DAT` (Encodings A, B,
  C), `_CHROMS.INF`, and `_CHROnnnn.DAT`.
- mzML export for MS functions.
- Corpus-validated against QTOF Ultima, SYNAPT G2-Si (IMS), and
  Xevo G2-XS QTof.
