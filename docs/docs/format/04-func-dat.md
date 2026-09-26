# _FUNCnnn.DAT

Binary spectrum data file. One file per function.
Contains all spectra for that function, stored contiguously.
Several record formats are observed depending on index variant and acquisition software.

## Encoding A: 6-byte records (Variant A index)

### Status: Decoded; m/z checked against lock-mass references (2026-09-26)

Observed in: PXD058812, PXD003126, PXD010569, PXD021125, PXD029515,
PXD041695 (older QTof and Q-Tof Premier-class instruments, MassLynx 4.0-4.1)

Key facts:
- File is a flat array of 6-byte records (no top-level file header)
- Scan boundaries and record counts come from IDX Variant A (offset u32@0x00,
  count in the low 24 bits of u32@0x04)
- Each record stores an ion count and a floating-point m/z word; there are
  no sentinel records and no TOF bins to rescale

### 6-byte Record Layout

| Bytes | Type    | Description |
|-------|---------|-------------|
| 0-1   | u16 LE  | Ion count (TDC hits; small integers, 0 for range markers) |
| 2     | u8      | High nibble: m/z exponent `e`. Low nibble: always 0 in the corpus |
| 3-5   | u24 LE  | m/z mantissa `M`, normalized (bit 23 always set) |

```
mz_uncal = M * 2^(e - 24)          # M in [2^23, 2^24), so mz_uncal in [2^(e-1), 2^e)
mz       = (T1(sqrt(mz_uncal)))^2  # _HEADER.TXT "Cal Function N" polynomial
```

The first and last records of a scan have count 0 and decode to the
acquisition range in `_FUNCTNS.INF`: in PXD058812 `70 CA FF C7` is
`0xC7FFCA * 2^-17 = 100.000` and `B0 4A FF F9` is
`0xF9FF4A * 2^-13 = 1999.98` for a 100-2000 function. The decoder rejects
records whose low nibble is non-zero or whose mantissa is not normalized.

Earlier versions read byte 2 as a "block type", byte 3 as an 8-bit intensity
and bytes 4-5 as a TOF bin anchored to `mz_high`. That reading put peaks
outside the acquisition range (for example 820-3312 in a 100-2000 function)
and is superseded.

### Validation (clean-room)

- Every record in six bundles has a zero low nibble and a normalized
  mantissa, and decoded ranges match `_FUNCTNS.INF` to within 0.03 Da.
- Lock-mass functions, after T1 calibration: PXD003126 [Glu1]-fibrinopeptide
  B [M+2H]2+ +24 ppm (+87 ppm without T1); PXD021125 +25 and +38 ppm.
- PXD041695 background ions calibrate to 429.092 and 445.12-445.13, matching
  the common polysiloxane contaminants at 429.0887 and 445.1200; without T1
  they are about 900 ppm high.
- PXD029515's lock function shows the Glu-fib envelope at a consistent
  -150 ppm on both isotopes, while PXD003126, acquired with identical
  `_extern.inf` geometry and software, sits at +24 ppm. The offset is
  attributed to that instrument's calibration at acquisition time; this has
  not been proven.

## Encoding D: 8-byte records behind a Variant A index

### Status: Decoded; m/z checked against physics and lock-mass references (2026-09-26)

Observed in: PXD081045 (Vion IMS QTof, UNIFI 2.0 export), PXD001123 and
PXD009047 (SYNAPT G2, MassLynx 4.1), PXD037102

Key facts:
- 22-byte IDX Variant A index; consecutive offsets divided by record counts
  give an 8-byte record width
- No sentinel records; the first and last records are ordinary profile points

### 8-byte Record Layout

| Bytes | Type    | Description |
|-------|---------|-------------|
| 0-3   | u32 LE  | Intensity, unsigned 16.16 fixed point (bytes 0-1 are dyadic fractions such as 1/2, 1/4, 3/4) |
| 4-7   | u32 LE  | m/z word: 5-bit exponent `E` (bits 27-31), 27-bit mantissa `F` with bit 26 always set |

```
mz_uncal = F * 2^(E - 27)          # F in [2^26, 2^27)
mz       = (T1(sqrt(mz_uncal)))^2
```

### Validation (clean-room)

- Profile points are spaced by exactly one ADC sample. With flight time
  `t = A_us * sqrt(m/z)` from `Lteff`/`Veff` and the `ADC Sample Frequency`
  in `_extern.inf`, one sample is `2 * sqrt(m/z) * A_us / f`. The median
  observed spacing divided by that prediction is 0.9986-1.0028 in every
  100-Da band from 200 to 1900 m/z (PXD081045, 7.2 GHz) and 0.9998 on the
  SYNAPT G2 files (3.0 GHz). A linear TOF-bin reading cannot satisfy this.
- Isotope spacing is flat at 4 words per Da over 256-512 m/z and 2 per Da
  over 512-1024, the signature of an m/z (not time) mantissa.
- Lock mass after T1: SYNAPT G2 Glu-fib and Leu-Enk at -10 and -11 ppm.
  Vion Leu-Enk (`ReferenceMass1` 556.27658 in `_extern.inf`) at +20 to
  +69 ppm across 45 runs, with one offset per acquisition batch; the intense
  monoisotopic peak saturates and centroids 10-20 ppm higher than its
  isotopes. Without T1 the same peaks are 130-146 ppm low.

The Vion files were previously decoded by the Encoding C first/last-record
anchor, which stretched each scan onto the declared mass range and put
Leu-Enk anywhere from -1447 to +3378 ppm.

## Encoding B: 8-byte records (IMS mode - SYNAPT G2-Si)

### Status: Decoded and Validated (Phase 4)

Observed in: PXD066594 (WANG.raw), PXD068881 (CtpA) - both SYNAPT G2-Si

Key facts:
- File is a flat array of 8-byte records with NO embedded scan headers
- Scan boundaries are given by IDX Variant B offsets (u32@0x16)
- Total: sum(scan record counts) x 8 = file size exactly (confirmed)
- Scan sizes vary (min 636,928 / max 784,640 bytes for WANG.raw) = variable ion detections
- Each 8-byte record represents one (IMS drift bin, TOF bin) cell with an ion count
- Each survey scan is a complete 2D IMS-TOF image: every occupied (dt_bin, tof_bin) cell is stored

### 8-byte Record Layout (IMS mode)

| Bytes | Type   | Confirmed | Description |
|-------|--------|-----------|-------------|
| 0     | u8     | Yes       | Always 0x00 in tested datasets |
| 1     | u8     | Yes       | Always 0x00 (padding) |
| 2-3   | u16 LE | **Yes**   | Ion count (TDC count per cell; 0-~800 typical) |
| 4-5   | u16 LE | **Yes**   | dt_bin: IMS drift time bin (see below) |
| 6-7   | u16 LE | **Yes**   | tof_bin: TOF time bin (same role as Encoding C bytes[6-7]) |

Sort key = `(tof_bin << 16) | dt_bin`, ascending. Records are sorted primarily by tof_bin
(= m/z) then by dt_bin (= IMS drift position) within each tof_bin group.

Note: in previous analysis, bytes[1:4] were incorrectly treated as a u24 intensity.
The correct layout has intensity as u16 at bytes[2:4], with bytes[0:2] always zero.

### IMS Drift Time Encoding

The dt_bin (bytes[4:6]) encodes IMS drift time linearly within the scan window:

```
drift_time_ms = dt_bin * scan_time_ms / 65536
```

where scan_time_ms = scan_time in ms from _FUNCTNS.INF +0x020 (× 1000).

The IMS grid is sparse relative to the push count: instruments use N_IMS equally-spaced
drift bins covering the full scan duration.

| Dataset | scan_time | dt_bin step | N_IMS bins | IMS bin width |
|---------|-----------|-------------|------------|---------------|
| WANG    | 1000 ms   | 912         | 71         | 13.9 ms       |
| CtpA    | 300 ms    | ~4928       | 13         | 22.6 ms       |

The dt_bin value for each cell is FIXED across all scans (does not change with RT).
Only the ion count at that cell varies scan-to-scan.

Cross-validated: WANG _PROC003.DAT dt_bin field uses the same 1712-unit spacing
as the raw _FUNC001.DAT dt_bin field, confirming they are the same IMS coordinate.

### Sentinel Records

- CtpA: has TWO zero-count sentinel records (first and last in scan, same as Encoding C).
  First sentinel tof_bin = tof_bin_low (mz_low anchor); last sentinel = tof_bin_high.
- WANG: NO zero-count sentinels. First record tof_bin = tof_bin_low directly (no zero record).

For m/z decoding, tof_bin_low and tof_bin_high can always be derived from the first and last
records of any scan (sentinel or first real hit).

### TOF m/z Decoding (Encoding B)

Uses the tof_bin field (bytes[6:8]) only; dt_bin is NOT used for m/z.
Formula identical to Encoding C except sub_bin = 0 (integer tof_bin only, no sub-bin):

```
A_us         = sqrt(m_proton * Lteff_m / (2 * e * Veff)) * 1e6   [µs/sqrt(Da)]

# From first/last records of scan:
tof_bin_low  = tof_bin of first record in scan                    [integer]
tof_bin_high = tof_bin of last record in scan                     [integer]
t_low        = A_us * sqrt(mz_low)                                [µs]
t_high       = A_us * sqrt(mz_high)                               [µs]
t_bin        = (t_high - t_low) / (tof_bin_high - tof_bin_low)   [µs/bin]

# For each data record:
t_raw_us     = t_low + (tof_bin - tof_bin_low) * t_bin            [µs]
t_cal_us     = c0 + c1*t_raw + c2*t_raw^2 + ...                   [T1 polynomial]
mz           = (t_cal_us / A_us)^2                                 [Da]
```

Note: m/z precision is limited to integer tof_bin (no sub-bin fractional correction).
At 4.6-5.6 ns/bin, resolution at mz=500 Da is ~0.10 Da per bin.

Validated: CtpA scan 228 (RT=2.4268 min).
Expected m/z=122.08 (from Apex3DIons.csv accession), decoded mz_raw≈121.67-122.03 from
records at tof_bin=16191-16195 using the sentinel-derived t_bin=4.62 ns. After
calibration polynomial the decoded m/z converges to the Apex3D-reported value.

## Encoding C: 8-byte records (non-IMS QTof mode - Xevo G2-XS)

### Status: Decoded and Validated (Phase 3)

Observed in: PXD075602 (DHPR_11257-1.raw, Xevo G2-XS QTof)

Key facts:
- Same 30-byte IDX Variant B as IMS datasets; DAT offsets at IDX+0x16
- Same 8-byte record size as Encoding B, but structurally different internal layout
- Scan sizes range from 5,776 to 1,019,888 bytes (722-127,486 records per scan)
- Records are sorted ascending by compound coordinate (bytes 4-7)
- Bytes[0-1] are **always 0x0000** (no flags, no drift time - non-IMS instrument)
- Every scan has a fixed FIRST record (zero intensity, encodes mz_low bound)
  and a fixed LAST record (zero intensity, encodes mz_high bound)

### 8-byte Record Layout (non-IMS QTof mode)

| Bytes | Type   | Confirmed | Description |
|-------|--------|-----------|-------------|
| 0-1   | u16 LE | Yes       | Always 0 (no drift-time axis for non-IMS instruments) |
| 2-3   | u16 LE | Yes       | Intensity (16-bit unsigned; 0-~500 range typical) |
| 4-5   | u16 LE | Yes       | Sub-bin: fine TDC position within the coarse TOF bin (fractional offset, 0-65535) |
| 6-7   | u16 LE | Yes       | tof_bin: coarse TOF time-bin index |

The sort key compound u32 = `(tof_bin << 16) | sub_bin` (ascending). Records are sorted
primarily by tof_bin (coarse position), then sub_bin (fine position) within each tof_bin.

### Sentinel Records

- **First record** (always zero intensity): tof_bin = mz_low_bin, encodes start of the
  active detection window. Constant across all scans of the same function.
  Example (DHPR Fn1): tof_bin=13887 → corresponds to mz_low=50 Da.

- **Last record** (always zero intensity): tof_bin = mz_high_bin, encodes end of the
  active detection window. Constant across all scans.
  Example (DHPR Fn1): tof_bin=23727 → corresponds to mz_high=1200 Da.

The sentinel pair provides the linear calibration anchor for converting tof_bin to flight time.

### TOF m/z Decoding (Encoding C)

Calibration constants: `_HEADER.TXT` (Cal Function N polynomial), `_extern.inf` (Lteff, Veff),
`_FUNCTNS.INF` (mz_low at +0x0A0, mz_high at +0x120).

```
A_us         = (Lteff_mm / 1000) / sqrt(2 * e_per_Da * Veff) * 1e6   [µs/sqrt(Da)]

# From sentinel records:
mz_low_bin   = tof_bin of first record in scan                        [integer]
mz_high_bin  = tof_bin of last record in scan                         [integer]
t_low        = A_us * sqrt(mz_low)                                    [µs]
t_high       = A_us * sqrt(mz_high)                                   [µs]
t_bin        = (t_high - t_low) / (mz_high_bin - mz_low_bin)         [µs/bin]

# For each data record:
frac_bin     = tof_bin - mz_low_bin + sub_bin / 65536                 [bins, fractional]
t_raw_us     = t_low + frac_bin * t_bin                               [µs]
t_cal_us     = c0 + c1*t_raw + c2*t_raw^2 + ...                      [T1 polynomial]
mz           = (t_cal_us / A_us)^2                                    [Da]
```

Validated: DHPR_11257-1.raw scan 575 (RT=10.022 min).
Top peaks at m/z ≈ 591, 608, 809, 822, 881 Da - consistent with LC-MS tryptic peptides
at mid-gradient in a 20-minute LC run.

### Distinguishing Encoding B from C

Both encodings use 8-byte records and IDX Variant B (30-byte stride).
The presence of IMS data can be confirmed by:
- `Apex3DIons.csv` in the `.raw` folder (IMS only, if Apex3D processing was run)
- `_FUNCTNS.INF` scan_subtype byte +0x01: 0x71 = IMS survey, 0xF1 = IMS lock-mass
- Record structure: Encoding B bytes[0:2] = 0x0000 always; Encoding C also always 0x0000.
  Distinguishing B from C requires checking whether bytes[4:6] (lo) covers the full 0-65535
  range uniformly (B = IMS bins, typically 13-71 fixed positions) vs varying per record (C = sub_bin).

In practice, IMS datasets always have `_PROC*.DAT/IDX/STS` files or Apex3D output files.

## Fields Under Investigation

- Encoding A: meaning of the byte 2 low nibble (always zero in the corpus)
- Encodings B and C: bytes 4-7 pass the same floating-point m/z and ADC-sample
  checks as Encoding D in every 30-byte-index function sampled, and bytes 0-1
  look like fractional intensity rather than a drift axis. The first/last
  record anchor used by the current B and C decoders fails lock-mass checks.
- Encoding B: whether byte[0] ever takes non-zero values and what they encode

## Reference Sources

- Empirical hex analysis: `re/src/analysis/inspect.py`
- Calibration: `_extern.inf` (Lteff, Veff, pusher cycle) + `_HEADER.TXT` (Cal Function N)
- Corpus samples:
  - PXD058812/molecular_mass_P15_01.raw (Encoding A, 197 scans, ~1050 rec/scan)
  - PXD058812/MS_fragmentation_P29_01.raw (Encoding A, 426 scans)
  - PXD066594/WANG.raw (Encoding B, 590 scans, 79616-98080 rec/scan)
  - PXD068881/20220517_CtpA_1076_2h_1.raw (Encoding B, 1138 scans)
  - PXD075602/DHPR_11257-1.raw (Encoding C, 1150 scans, 722-127486 rec/scan)
