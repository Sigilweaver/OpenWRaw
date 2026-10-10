---
sidebar_position: 1
slug: /
---

# OpenWRaw

:::info Part of the OpenMassSpec stack

OpenWRaw is one of the vendor readers in
[OpenMassSpec](https://sigilweaver.app/openmassspec/docs/), a Rust- and
Python-native stack for proteomics raw-file access. Sibling readers:
[OpenTFRaw](https://sigilweaver.app/opentfraw/docs/) (Thermo `.raw`),
[OpenTimsTDF](https://sigilweaver.app/OpenTimsTDF/docs/) (Bruker `.d/`).

:::

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
| `_FUNCnnn.IDX` Variant B (30-byte)              | supported  |
| `_FUNCnnn.DAT` Encoding A (6-byte records)      | supported  |
| `_FUNCnnn.DAT` Encoding D (8-byte floating m/z) | supported  |
| `_FUNCnnn.DAT` Encoding E (12-byte records)     | supported for public LCT Premier and Xevo G2-XS lock-mass fixtures; flags/auxiliary word unresolved |
| Ion mobility (drift time)                       | not decoded |
| Lock-mass correction                            | not applied |
| `_CHROMS.INF` + `_CHROnnnn.DAT` chromatograms   | supported  |
| mzML export                                     | supported  |
| Apex3D `.bin` files                             | best-effort|

Validated instrument classes:

| Instrument class              | Encoding | IDX Variant |
| ----------------------------- | -------- | ----------- |
| QTOF Ultima                   | A        | A           |
| Vion (UNIFI export), SYNAPT G2 | D       | A           |
| SYNAPT G2-S / G2-Si / XS      | D        | B           |
| Xevo G2-XS QTof, Xevo G3      | D (some lock-mass functions E) | B |

SYNAPT scans are decoded as m/z and intensity only; ion mobility is not
decoded.

## Next steps

- [Install](./install) the Rust crate or the Python package.
- Run through the [Quickstart](./quickstart).
- Read the [Format specification](./format/overview) for the directory
  layout and per-file binary layouts.
- Browse the API on [docs.rs](https://docs.rs/openwraw).

## License

OpenWRaw is Apache-2.0 licensed. See [License](./license).
