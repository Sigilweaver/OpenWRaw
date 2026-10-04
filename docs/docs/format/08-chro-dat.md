# _CHROnnnn.DAT

Binary chromatogram data file. One file per recorded channel (e.g.
`_CHRO001.DAT`, `_CHRO002.DAT`). Contains a time-series of (RT, value)
pairs for a single instrument channel (pump pressure, flow rate,
temperature, etc.). The channels are described by `_CHROMS.INF`.

## Status: Fully Decoded

## File Layout

The file uses the same Waters Parameter Table structure as `_FUNCnnn.STS`:

```
[32-byte file preamble]
[2 × 48-byte descriptor records]    (always exactly 2)
[n_records × 8-byte data records]
```

Total header = 32 + 2 × 48 = 128 bytes. `data_offset` at preamble byte 0
is always 0x80 = 128.

## File Preamble (32 bytes)

| Offset | Type | Value | Description |
|--------|------|-------|-------------|
| 0x00 | u16 | 0x0080 (128) | data_offset - byte offset to first data record |
| 0x02 | u16 | 1 | Format version |
| 0x04 | u16 | 8 | Bytes per data record |
| 0x06 | u16 | 2 | Number of descriptor records (always 2) |
| 0x08-0x1F | - | zeroes | Padding |

## Descriptor Records

Both are always present with the same structure (unk=3 = f32 encoding):

| Index | seq | name | code | encoding | Description |
|-------|-----|------|------|----------|-------------|
| 0 | 1 | `Time` | 0 | f32 | Retention time (minutes) |
| 1 | 2 | `Intensity` | 4 | f32 | Channel value (units from `_CHROMS.INF` `$CC$` string) |

## Data Records (8 bytes each)

| Offset | Type | Description |
|--------|------|-------------|
| 0 | f32 | Retention time (minutes) |
| 4 | f32 | Channel value in instrument-specific units |

The sampling rate is independent of the MS scan rate. LC channels are
typically sampled at 1-10 Hz; the number of CHRO records per MS scan
varies accordingly.

## CHRO File Numbering

CHRO files are numbered 1-based and correspond to the 85-byte channel records
in `_CHROMS.INF`, starting immediately after its 128-byte descriptor header.
The Flags and Description descriptors are fields inside that header and do
not have companion CHRO files.

| CHROMS.INF channel index | CHRO file |
|-------------------------|-----------|
| 0 | `_CHRO001.DAT` |
| 1 | `_CHRO002.DAT` |
| 2 | `_CHRO003.DAT` |

## Observed Values

| File | Dataset | Channel | n_records | RT range (min) | Value range |
|------|---------|---------|-----------|----------------|-------------|
| CHRO001 | CtpA.raw | BSM System Pressure | 7201 | 0.002-12.002 | 4753-6782 |
| CHRO002 | CtpA.raw | BSM Composition A | 721 | 0.017-12.017 | 94.7-95.0 (%) |

The first channel in the public 213-byte PSU metadata file is column
temperature. Its `_CHRO001.DAT` has 900 points, starting at 0 minutes with
approximately 40 degrees C, consistent with the channel name and units.
The four-channel MTBLS701 LCT Premier acquisition maps pressure, composition,
sample temperature and column temperature to CHRO001..CHRO004 in that order.

## Units Lookup

The units for each channel's value field are stored in `_CHROMS.INF` as the
`$CC$` spec string for the matching record. See [05-chroms-inf.md](05-chroms-inf.md)
for the `$CC$` parsing rules.

## Reference Sources

- Corpus samples:
  - PXD068881/20220517_CtpA_1076_2h_1.raw (7 CHRO files, 7 channels)
  - PXD075602/DHPR_11257-1.raw (5 CHRO files, 5 channels)
