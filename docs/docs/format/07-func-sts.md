# _FUNCnnn.STS

Binary scan statistics file. One file per function (e.g. `_FUNC001.STS`,
`_FUNC002.STS`). Records per-scan instrument housekeeping values: electrode
voltages, collision energies, push counts, lock-mass corrections, and TIC
traces.

## Status: Fully Decoded

## File Layout

```
[32-byte file preamble]
[n_desc × 48-byte descriptor records]      (starting at offset 0x20)
[n_scans × scan_record_size bytes of data]
```

Total header size = 32 + n_desc × 48. This equals the `data_offset` stored
at preamble byte 0.

## File Preamble (32 bytes)

| Offset | Type | Description |
|--------|------|-------------|
| 0x00   | u16  | `data_offset` - byte offset where per-scan data begins |
| 0x02   | u16  | Format version (always 1 observed) |
| 0x04   | u16  | `scan_record_size` - bytes per scan in the data section |
| 0x06   | u16  | `n_desc` - number of descriptor records |
| 0x08-0x1F | - | Zeroes (padding to 32 bytes) |

Equations:
- `data_offset = 32 + n_desc * 48`
- `n_scans = (file_size - data_offset) / scan_record_size`

Validated:
- PXD058812 P15: data_offset=0x4D0=1232, scan_sz=63, n_desc=25, n_scans=197
- PXD075602 DHPR: data_offset=0x0AA0=2720, scan_sz=167, n_desc=56, n_scans=1150

## Descriptor Records (48 bytes each)

Each descriptor defines one channel stored in the per-scan data section.

| Offset within record | Type | Description |
|---------------------|------|-------------|
| 0x00 | u16 | Channel sequence number (not always contiguous; some skip) |
| 0x02 | u16 | Encoding type (see table below) |
| 0x04 | u16 | Byte offset of this channel within each scan record |
| 0x06-0x2F | bytes | Null-padded ASCII channel name (42 bytes) |

### Encoding Types

| Type code | C type | Size | Notes |
|-----------|--------|------|-------|
| 0 | u8 | 1 byte | Boolean flag / small integer |
| 1 | i16 | 2 bytes | Signed 16-bit integer |
| 2 | u32 | 4 bytes | Unsigned 32-bit integer |
| 3 | f32 | 4 bytes | IEEE 754 single-precision float |

The byte offset in the descriptor (`0x04`) is used directly as an index
into the per-scan record. Channels are packed without explicit gaps; the
offset of channel k+1 equals offset k + size(type k).

## Per-Scan Data

The data section starts at `data_offset` and contains `n_scans` consecutive
records, each `scan_record_size` bytes. To read channel X for scan S:

```
channel_value = scan_data[S * scan_record_size + descriptor[X].offset]
```

decoded using the descriptor's encoding type.

## Known Channels

Observed across all four corpus datasets. Channels available depend on
instrument generation; newer firmware adds channels.

| Seq | Name | Type | Code | Notes |
|-----|------|------|------|-------|
| 13 | Segment Number | i16 | varies | 0 for normal scans |
| 52 | Cone | i16 | varies | Cone voltage (V); matches `_extern.inf` Sampling Cone |
| 62 | Collision Energy | f32 | varies | Per-scan collision energy (eV) |
| 76/78 | Pusher Frequency / Collision Energy2 | varies | varies | Function-dependent |
| 100 | Reference Scan | u8 | varies | 1 if this scan is a lock-mass reference |
| 101 | Use lock mass correction | u8 | varies | Boolean |
| 102 | Lock mass correction | f32 | varies | Mass correction applied (Da) |
| 103 | Use temp correction | u8 | varies | Boolean |
| 104 | Temperature correction | f32 | varies | Temperature-derived TOF correction (µs) |
| 107 | TIC Trace A | f32 | varies | Total Ion Current (a.u.) for function A |
| 108 | TIC Trace B | f32 | varies | Total Ion Current (a.u.) for function B |
| 116 | Scan Push Count | u32 | varies | Number of ADC pushes summed in this scan |
| 117 | Scanwave Normalisation Factor | f32 | varies | IMS normalisation factor |
| 121 | ETD Fragmentation Mode | i16 | varies | 0 = CID; non-zero = ETD |

Not all channels are present in every file. Parse by looking up the
descriptor list rather than assuming fixed offsets.

## Observed Values

| Dataset | n_desc | scan_sz | Cone | Collision Energy | Push Count |
|---------|--------|---------|------|-----------------|------------|
| PXD058812 P15 (non-IMS QTOF) | 25 | 63 | 30 V | 10.0 eV | - |
| PXD075602 DHPR (Xevo G2-XS) | 56 | 167 | 30 V | 4.0 eV | 8299 |

Push Count = 8299 in DHPR: 8299 × 60.25 µs ≈ 0.50 s per scan, consistent
with the 0.50 s scan time recorded in `_FUNCTNS.INF`.

## Relationship to IDX Fields

The IDX `+0x08` field (30-byte Variant B, value labelled "Large value / TIC?")
likely corresponds to one of the TIC Trace channels here. This cross-check
has not yet been confirmed empirically.

## Rust Implementation

`crate::raw::func_sts::FuncSts` parses this file and exposes per-scan
channel lookups by name; `FuncSts::collision_energy(scan_idx)` reads the
"Collision Energy" channel specifically. `Reader::open` parses every
function's `_FUNCnnn.STS` when present (silently `None` if absent or
malformed - this file is supplementary, not required to decode peak data),
and `mzml::precursor_info_for` uses it to populate
`PrecursorInfo::collision_energy` for every MS2 function, including MSe/HDMSe
(which has no discrete precursor `target_mz` but still reports a real
per-scan collision energy) and targeted MS/MS (Sigilweaver/OpenWRaw#8, #13).

## Reference Sources

- Empirical analysis: all `_FUNC001.STS` files in corpus
- Corpus samples:
  - PXD058812/molecular_mass_P15_01.raw (25 channels, 63 bytes/scan)
  - PXD075602/DHPR_11257-1.raw (56 channels, 167 bytes/scan)
  - PXD035818/17122018_TNFA_PEPTIDE_GSHH_MSMS_884.raw (targeted MS/MS,
    confirms the same descriptor/channel layout on a non-MSe function)
