---
sidebar_position: 2
---

# Encodings

Spectrum data in `_FUNCnnn.DAT` uses one of three encodings,
correlated with the IDX variant and instrument class:

| Encoding | Record size | IDX Variant | Typical instrument          |
| -------- | ----------- | ----------- | --------------------------- |
| A        | 6 bytes     | A (22-byte) | QTOF Ultima (older)         |
| B        | 8 bytes     | B (30-byte) | SYNAPT G2-Si (IMS)          |
| C        | 8 bytes     | B (30-byte) | Xevo G2-XS QTof             |

The decoder is selected per function based on the `ChannelType` and
record-size signature surfaced by `FunctionTable`. The `data` module
exposes `decode_encoding_a`, `decode_encoding_b`, and `decode_encoding_c`,
all of which return an `MsSpectrum { mz: Vec<f64>, intensity: Vec<f32> }`
after applying the calibration polynomial from `_HEADER.TXT`.

See the [format specification](../format/func-dat) for byte-level
layouts.
