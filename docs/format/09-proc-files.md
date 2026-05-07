# _PROCnnn.DAT / _PROCnnn.IDX / _PROCnnn.STS / _PROCnnn.MAX

Post-processed spectrum data created by MassLynx peak-processing step.
Found only in SYNAPT G2-Si IMS acquisitions in the corpus; not present in
non-IMS or native-MS-only datasets.

## Status: Partially Decoded (Phase 3)

Core structure decoded. MAX file internals and PROC number assignment scheme
are not fully understood.

## Corpus Presence

| Dataset | PROC files | Notes |
|---------|-----------|-------|
| PXD058812 (Q-TOF Ultima, non-IMS) | None | |
| PXD066594 (WANG.raw, SYNAPT G2-Si, IMS) | PROC 002,003,005,006,008,009 | |
| PXD068881 (CtpA, SYNAPT G2-Si, IMS) | None | |
| PXD075602 (DHPR, Xevo G2-XS, non-IMS) | None | |

CtpA is also SYNAPT G2-Si but has no PROC files; PROC file generation is
apparently optional or controlled by MassLynx processing settings.

## PROC Number Scheme

PROC numbers observed: 2, 3, 5, 6, 8, 9 (no 1, 4, 7).
Numbers appear to be assigned in pairs sharing the same retention time:

| PROC pair | RT (min) | DAT size | MAX file |
|-----------|----------|----------|---------|
| 002 + 003 | 0.2027   | 120000 + 551772 | PROC002.MAX |
| 005 + 006 | 1.3864   | 120000 + 544488 | PROC005.MAX |
| 008 + 009 | 1.3864   | 120000 + 540948 | PROC008.MAX |

Within each pair, one file has a fixed 120000-byte DAT (with MAX) and one
has a variable-size DAT (without MAX). The two members appear to represent
complementary projections or resolution levels of the same IMS-MS data.

The PROC numbers skip 1, 4, 7; these gaps may correspond to `_FUNC001`
and to IMS reference functions that are not processed into PROC files.

## _PROCnnn.IDX

30-byte records, same stride as _FUNCnnn.IDX Variant B. Always exactly one
record per PROC file (one "processed spectrum" per file).

| Offset | Type | Description |
|--------|------|-------------|
| 0x00   | u32  | Flags (always 0) |
| 0x04   | u16  | Record count (= n_records in paired .DAT) |
| 0x06   | u8   | Type byte: 0x78 for fixed-size DAT (with MAX), 0x58 for variable-size DAT |
| 0x07   | u8   | 0x00 |
| 0x08   | u32  | Hardware counter; pairs at same RT share high 2 bytes |
| 0x0C   | f32  | Retention time of the processed spectrum (minutes) |
| 0x10   | u16  | Scan-varying count (13945-27076 in WANG corpus); purpose unclear; may be a peak count or hardware reference |
| 0x12   | u16  | Small for fixed-size type (738-741 in corpus), large for variable-size (21680-50355); purpose unclear |
| 0x14   | u16  | Same value for PROC pairs at identical RT; differs between RT points (34646-56336 in corpus); may be a scan-level TIC or hardware register |
| 0x16   | u32  | DAT byte offset (always 0; each PROC has its own .DAT file) |
| 0x1A   | u32  | 0 |

## _PROCnnn.DAT

12-byte records packed sequentially with no header. Record count is given
by u16@0x04 in the paired .IDX file.

```
struct ProcRecord {
    u32 intensity;      // raw intensity (varies)
    u32 position;       // packed (mz_bin << 16) | dt_bin
    u32 tag;            // 0x40000000 (= f32 2.0) when intensity > 0, else 0
};
```

### Fixed-size DAT (002/005/008 type, with MAX)

10000 records. Coarse 2D grid of mz x drift:
- 1250 unique mz_bin values
- 8 dt_bin steps per mz_bin, spaced 0x2000 (= 8192) apart
- dt_bin range: 0x0000 to 0xE000 (step 0x2000)

Position field decoding:
```
mz_bin = position >> 16
dt_bin = position & 0xFFFF
```

The 1250 mz_bins form a contiguous range starting from a calibration-dependent
offset (e.g. mz_bin=29922 for PROC002 with RT=0.2027 in WANG.raw).

### Variable-size DAT (003/006/009 type, no MAX)

~45000 records per file. Finer dt_bin spacing (step = 1712 vs 8192),
more mz_bins covered:
- First observed mz_bin = 22278 (smaller than fixed-type start of 29922)
- dt_bin increments of 0x06B0 = 1712

The variable-size type likely represents the full-resolution IMS-MS peak list
while the fixed-size type is a downsampled or projected version.

## _PROCnnn.STS

Waters Parameter Table format (same as `_FUNCnnn.STS`). Always exactly one
data record per file (n_desc=54, rec_size=157 bytes, n_records=1).

The 54 channels include both standard ESI/TOF fields (Cone, Collision Energy,
TIC Trace A/B, Scan Push Count, Lock mass correction, Pusher Frequency) and
MALDI-specific legacy fields (Linear Detector Voltage, Laser Repetition Rate,
Laser Energy Coarse/Fine, Aim X/Y Position, Sample Plate Voltage, PSD fields,
Source Region 1/2). The MALDI fields appear to be a fixed schema present in
all Waters STS files regardless of instrument type; their values in an ESI
dataset are expected to be zero or default.

See `07-func-sts.md` for the Waters Parameter Table format description.

Notable channels in PROC STS:
- `TIC Trace A/B` (f32): TIC for this processed spectrum
- `Scan Push Count` (u32): number of IMS pushes summed into this spectrum
- `Collision Energy` (f32): collision energy used
- `Cone` (i16): cone voltage
- `PIC Function Number` (f32): back-reference to source function
- `PIC Channel Number` (f32): back-reference to source channel
- `PIC Retention Time` (f32): RT of peak apex

## _PROCnnn.MAX

Present only alongside fixed-size DAT files (002/005/008 type). File sizes:
1105024, 1090456, 1083376 bytes.

Structure:
- Bytes 0-3: f32 = record count of the paired .DAT file (e.g. 10000.0)
- Bytes 4-7: zeros
- Remaining data: purpose unknown; likely full-resolution IMS mobility
  profiles for each (mz_bin, dt_bin) pair, enabling maximal projection
  along the drift axis

The .MAX suffix and the "maximum" naming convention in MassLynx processing
suggest this stores the maximum-intensity projection data from Apex3D or
a related peak-detection algorithm.

## Interpretation

PROC files represent post-processed "peak apex" spectra extracted by
MassLynx from the raw `_FUNC001.DAT` IMS data. Each PROC file corresponds
to one discrete retention time of interest (e.g. a detected chromatographic
peak). The PROC number does not correspond to a function number in
`_FUNCTNS.INF`; rather, it is an internal MassLynx index assigned during
data processing.

The paired structure (fixed + variable size at the same RT) suggests:
- Fixed-size / MAX: coarse Apex3D projection grid for visualization
- Variable-size: full list of detected (drift, mz) peak centroids
