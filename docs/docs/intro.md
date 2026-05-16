---
sidebar_position: 1
slug: /
---

# OpenWRaw

OpenWRaw is a Rust library that reads Waters MassLynx `.raw`
acquisition directories - the directory-based format produced by
Waters LC-MS instruments (Synapt, Xevo, ACQUITY, and related product
lines).

It runs on Linux, macOS, and Windows with no native or system
dependencies. The format was decoded by binary analysis of a corpus of
public mass-spectrometry datasets (PRIDE accessions).

Optional Python bindings are available via the
[`openwraw`](./install) wheel.

## What it covers

| Component                                       | Status     |
| ----------------------------------------------- | ---------- |
| `_HEADER.TXT` (metadata + calibration)          | supported  |
| `_extern.inf` (instrument geometry)             | supported  |
| `_FUNCTNS.INF` (function descriptors)           | supported  |
| `_FUNCnnn.IDX` Variant A (22-byte)              | supported  |
| `_FUNCnnn.IDX` Variant B (30-byte, IMS-capable) | supported  |
| `_FUNCnnn.DAT` Encoding A (6-byte records)      | supported  |
| `_FUNCnnn.DAT` Encoding B (8-byte IMS cells)    | supported  |
| `_FUNCnnn.DAT` Encoding C (8-byte sub-bin)      | supported  |
| `_CHROMS.INF` + `_CHROnnnn.DAT` chromatograms   | supported  |
| mzML export                                     | supported  |
| Apex3D `.bin` files                             | best-effort|

Validated instrument classes:

| Instrument class    | Encoding | IDX Variant |
| ------------------- | -------- | ----------- |
| QTOF Ultima         | A        | A           |
| SYNAPT G2-Si (IMS)  | B        | B           |
| Xevo G2-XS QTof     | C        | B           |

## Next steps

- [Install](./install) the Rust crate or the Python package.
- Run through the [Quickstart](./quickstart).
- Read the [Format specification](./format/overview) for the directory
  layout and per-file binary layouts.
- Browse the API on [docs.rs](https://docs.rs/openwraw).

## License

OpenWRaw is Apache-2.0 licensed. See [License](./license).
