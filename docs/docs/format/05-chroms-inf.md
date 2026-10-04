# _CHROMS.INF

Binary instrument channel descriptor file. Present when LC/pump/analog channels
are recorded alongside the mass spectrometry data (typically on LC-MS systems).
Absent in direct-infusion or pure-MS datasets.

## Status: Fully Decoded

Observed in: PXD068881 (CtpA, SYNAPT G2-Si with LC), PXD075602
(DHPR_11257-1.raw, Xevo G2-XS with LC), MTBLS701 (LCT Premier), and
MSV000083877 (Xevo G2 QTof, one fluorescence channel).

## File Layout

```
[32-byte preamble]
[48-byte field descriptor 0: Flags]
[48-byte field descriptor 1: Description]
[85-byte channel record 0]
[85-byte channel record 1]
...
```

File size = 128 + N_channels * 85. The two field descriptors are inside the
128-byte header. They are not additional channel records after the header.
A valid one-channel file is therefore 213 bytes, not at least 298 bytes.

Validated from native public bytes:

- MSV000083877/ROF_181101_04_IgG.raw: 213 bytes, one fluorescence channel.
- PSU Data Commons/kt130808_WAS_0179.raw: 213 bytes, one column temperature channel.
- MTBLS701/1506_SZ_SZ_E01_neg.raw: 468 bytes, four channels.
- PXD068881/20220517_CtpA_1076_2h_1.raw: 723 bytes, seven channels.
- PXD075602/DHPR_11257-1.raw: 553 bytes, five channels.

## Preamble and Field Descriptors (128 bytes total)

| Offset | Type | Value | Description |
|--------|------|-------|-------------|
| 0x00 | u16 | 128 | Offset of the first channel record |
| 0x02 | u16 | 1 | Format version |
| 0x04 | u16 | 85 | Channel record size in bytes |
| 0x06 | u16 | 2 | Number of 48-byte field descriptors inside the header |
| 0x08-0x1F | bytes | zeroes | Preamble padding |
| 0x20-0x4F | bytes | - | Flags descriptor |
| 0x50-0x7F | bytes | - | Description descriptor |

Each field descriptor contains a sequence number (`u16` at relative offset 0),
encoding code (`u16` at 2), record-field offset (`u16` at 4), null-padded name
(bytes 6..31), and field width (`u16` at 32). Observed descriptors are:

| Field | Sequence | Encoding code | Offset within channel record | Width |
|-------|----------|---------------|------------------------------|-------|
| Flags | 1 | 2 | 0 | 4 |
| Description | 2 | 5 | 4 | 81 |

## Channel Records (85 bytes each)

Channel records begin at byte 128. Channel record 0 corresponds to
`_CHRO001.DAT`, record 1 to `_CHRO002.DAT`, and so on.

Each record describes one recorded chromatographic channel.

| Offset | Type | Confirmed | Description |
|--------|------|-----------|-------------|
| 0x00   | u32  | **Yes**   | Source device type (4 = BSM pump, 1 = column/sample device) |
| 0x04   | bytes | **Yes**  | Null-padded ASCII channel name (Windows-1252; may start with 0xB5 = µ) |
| ...    | str  | **Yes**   | `$CC$` spec string (null-terminated, at end of record) |

The channel name and `$CC$` string are packed into bytes 0x04-0x54 in sequence,
separated by a null byte between them.

### `$CC$` Spec String Format

```
$CC$,<scale_f>,<type_code>,<lo_limit>,<hi_limit>,<units>
```

- `scale_f` = float scale factor (e.g. 1.0 or 0.1)
- `type_code` = integer (always 3 in observed data)
- `lo_limit` = lower display limit (float)
- `hi_limit` = upper display limit (float)
- `units` = ASCII units string (e.g. `psi`, `%`, `uL/min`, `% Power`, `C`)

### Observed Channels (PXD068881 CtpA.raw - 7 channel records)

| Record | source_type | Channel Name | units |
|--------|-------------|--------------|-------|
| 0      | 4 (BSM)     | BSM System Pressure      | psi |
| 1      | 4 (BSM)     | BSM Composition A        | % |
| 2      | 4 (BSM)     | BSM Composition B        | % |
| 3      | 4 (BSM)     | BSM Measured Flow Rate A | µL/min |
| 4      | 4 (BSM)     | BSM Measured Flow Rate B | µL/min |
| 5      | 1 (col/samp)| (1) Peltier Engine Power | % Power |
| 6      | 1 (col/samp)| (1) Chamber Temp         | °C |

### Previously Documented Channel Subset (PXD075602 DHPR_11257-1.raw)

The following three channels are records 2..4 of the five-channel file.

| Record | source_type | Channel Name | units |
|--------|-------------|--------------|-------|
| 2      | 4 (BSM)     | BSM Measured Flow Rate B | µL/min |
| 3      | 4 (BSM)     | Column Temperature       | °C |
| 4      | 4 (BSM)     | Room Temp                | °C |

Note: BSM channel names are prefixed with 0xB5 (`µ` in Windows-1252) in the raw bytes.
The lo/hi fields in `$CC$` are 0,0 in all observed samples (limits may not be stored here).
Units encoding is Windows-1252: `°` is 0xB0, `µ` is 0xB5.

`source_type` is kept on `raw::chroms::ChromChannel` and exposed by the
Python `RawReader.channels` API (with binding coverage), so callers can
already filter or label BSM-pump vs. column/sample channels themselves. It is
deliberately not copied into `openmassspec_core::ChromatogramRecord`:
that model has no source-device field, and every channel already carries a
human-readable `name` that spells out its device (`"BSM ..."` vs.
`"(1) ..."`), so a numeric field would duplicate information the mzML
consumer can already get from the name. Inventing a CV annotation or
changing chromatogram selection based on the numeric device class would
still need a separately established PSI-MS mapping, which the corpus doesn't
motivate (Sigilweaver/OpenWRaw#24).

## Companion Chromatogram Files

For each channel record in `_CHROMS.INF` there is a corresponding `_CHROnnnn.DAT` file
numbered 1-based with 3-digit zero-padding (e.g., `_CHRO001.DAT` for record 0). The `.DAT` files contain decoded pairs of retention time and channel value;
see [08 - _CHROnnnn.DAT](08-chro-dat.md).

## Reference Sources

- Empirical hex analysis using `re/src/analysis/inspect.py`
- Corpus samples:
  - PXD068881/20220517_CtpA_1076_2h_1.raw (7 channels, 723 bytes)
  - PXD075602/DHPR_11257-1.raw (5 channels, 553 bytes)

The one-channel public sources and the corrected descriptor interpretation are
recorded in [ATTRIBUTION.md](https://github.com/Sigilweaver/OpenWRaw/blob/main/ATTRIBUTION.md).
