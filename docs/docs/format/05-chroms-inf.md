# _CHROMS.INF

Binary instrument channel descriptor file. Present when LC/pump/analog channels
are recorded alongside the mass spectrometry data (typically on LC-MS systems).
Absent in direct-infusion or pure-MS datasets.

## Status: Fully Decoded

Observed in: PXD068881 (CtpA, SYNAPT G2-Si with LC), PXD075602 (DHPR_11257-1.raw, Xevo G2-XS with LC)

## File Layout

```
[128-byte file header]
[85-byte meta record 0]    -- always type 1 (Flags)
[85-byte meta record 1]    -- always type 2 (Description)
[85-byte data record 0]
[85-byte data record 1]
...
[85-byte data record N-1]
```

File size = 128 + (2 + N_data) × 85

Validated: PXD068881 CtpA _CHROMS.INF = 128 + 7×85 = 723 bytes (7 records: 2 meta + 5 data)
Validated: PXD075602 DHPR_11257-1.raw _CHROMS.INF = 553 bytes = 128 + 5×85 (5 records: 2 meta + 3 data)

## File Header (128 bytes)

| Offset | Type | Value | Description |
|--------|------|-------|-------------|
| 0x00   | u16  | 0x0080 (128) | Header size in bytes |
| 0x02   | u16  | 1     | Format version (always 1 observed) |
| 0x04   | u16  | 0x0055 (85) | Record size in bytes |
| 0x06   | u16  | 2     | Number of meta records that follow (always 2 observed) |
| 0x08-0x7F | zeroes | - | Padding to 128 bytes |

## Meta Records (85 bytes each, always 2 records)

The two meta records immediately follow the header at offsets 0x80 and 0xD5.

| Offset within record | Type | Description |
|---------------------|------|-------------|
| 0x00                | u32  | Meta type: 1 = Flags, 2 = Description |
| 0x04-0x54           | bytes | Null-padded ASCII name string |

Observed names:
- Type 1 (Flags): `"Flags"`
- Type 2 (Description): `"Description"`

## Data Records (85 bytes each)

Data records follow immediately after the two meta records (at byte offset `128 + 2×85 = 298`).
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

### Observed Channels (PXD068881 CtpA.raw - 5 data records)

| Record | source_type | Channel Name | units |
|--------|-------------|--------------|-------|
| 0      | 4 (BSM)     | BSM Composition B        | % |
| 1      | 4 (BSM)     | BSM Measured Flow Rate A | µL/min |
| 2      | 4 (BSM)     | BSM Measured Flow Rate B | µL/min |
| 3      | 1 (col/samp)| (1) Peltier Engine Power | % Power |
| 4      | 1 (col/samp)| (1) Chamber Temp         | °C |

### Observed Channels (PXD075602 DHPR_11257-1.raw - 3 data records)

| Record | source_type | Channel Name | units |
|--------|-------------|--------------|-------|
| 0      | 4 (BSM)     | BSM Measured Flow Rate B | µL/min |
| 1      | 4 (BSM)     | Column Temperature       | °C |
| 2      | 4 (BSM)     | Room Temp                | °C |

Note: BSM channel names are prefixed with 0xB5 (`µ` in Windows-1252) in the raw bytes.
The lo/hi fields in `$CC$` are 0,0 in all observed samples (limits may not be stored here).
Units encoding is Windows-1252: `°` is 0xB0, `µ` is 0xB5.

## Companion Chromatogram Files

For each data record in `_CHROMS.INF` there is a corresponding `_CHROnnnn.DAT` file
numbered 1-based with 3-digit zero-padding (e.g., `_CHRO001.DAT` for record 0). The `.DAT` files contain the
time-series intensity data for each channel. The encoding of `_CHROnnnn.DAT` is not
yet decoded (but see `$CC$` scale factor for a clue to the units conversion).

## Reference Sources

- Empirical hex analysis using `re/src/analysis/inspect.py`
- Corpus samples:
  - PXD068881/20220517_CtpA_1076_2h_1.raw (7 records, 723 bytes)
  - PXD075602/DHPR_11257-1.raw (5 records, 553 bytes)
