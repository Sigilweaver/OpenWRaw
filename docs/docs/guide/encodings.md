---
sidebar_position: 2
---

# Encodings

Spectrum data in `_FUNCnnn.DAT` uses one of five encodings,
correlated with the IDX variant and instrument class:

| Encoding | Record size | IDX Variant | Typical instrument          |
| -------- | ----------- | ----------- | --------------------------- |
| A        | 6 bytes     | A (22-byte) | QTOF Ultima (older)         |
| B        | 8 bytes     | B (30-byte) | SYNAPT G2-Si (IMS)          |
| C        | 8 bytes     | B (30-byte) | Xevo G2-XS QTof             |
| D        | 8 bytes     | A (22-byte) | Vion / UNIFI export        |
| E        | 12 bytes    | A (22-byte) | LCT Premier                |

The reader selects the decoder from the IDX stride, consecutive DAT offsets
and record counts, and instrument class. The `raw::data` module exposes
`decode_encoding_a` through `decode_encoding_e`. Non-IMS decoders return
`Spectrum { mz: Vec<f64>, intensity: Vec<f32> }`; Encoding B returns
`ImsSpectrum` with a drift-time axis. Each uses the applicable calibration
polynomial from `_HEADER.TXT`.

Encoding E preserves flagged peaks but does not interpret its intensity
flags or auxiliary word. It was checked against original public MTBLS701
and MTBLS13770 bytes; this does not establish support for every LCT format.

See the [format specification](../format/func-dat) for byte-level
layouts.
