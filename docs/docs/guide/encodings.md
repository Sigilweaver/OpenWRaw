---
sidebar_position: 2
---

# Encodings

Spectrum data in `_FUNCnnn.DAT` uses one of three record layouts. The
encoding is a property of the DAT records, not of the index: Encodings D and
E appear behind both the 22-byte and the 30-byte `_FUNCnnn.IDX`.

| Encoding | Record size | IDX variant | Typical instruments |
| -------- | ----------- | ----------- | ------------------- |
| A        | 6 bytes     | A (22-byte) | Older QTOF, Q-Tof Premier class |
| D        | 8 bytes     | A or B      | Vion (UNIFI export), SYNAPT G2/G2-S/G2-Si/XS, Xevo G2-XS, Xevo G3 |
| E        | 12 bytes    | A or B      | LCT Premier; some Xevo G2-XS lock-mass functions |

`Reader::open` picks the encoding per function from the record width. With
the 22-byte index the width follows from record counts and consecutive
offsets. The 30-byte index has no record count, so the reader samples scans
and checks which width makes every record's bytes 4-7 a valid m/z word that
never decreases within the scan. The choice and its reason are logged at
debug level. The instrument name is not used.

The `raw::data` module exposes `decode_encoding_a`, `decode_encoding_d` and
`decode_encoding_e`. Each takes one scan's bytes and a `DecodeParams` built
with `DecodeParams::new(cal)`, and returns a `Spectrum` with `mz: Vec<f64>`
and `intensity: Vec<f32>`. Every encoding stores an uncalibrated m/z; the
`_HEADER.TXT` T1 polynomial is applied to sqrt(m/z). No lock-mass correction
is applied.

Encoding E preserves flagged peaks but does not interpret its intensity
flags or auxiliary word. It was checked against original public MTBLS701
and MTBLS13770 bytes and against the lock-mass reference in public Xevo
G2-XS bundles; this does not establish support for every LCT format.

See the [format specification](../format/func-dat) for byte-level
layouts.
