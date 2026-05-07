# _FUNCnnn.DAT

Binary spectrum data file. One file per function.
Contains all spectra for that function, stored contiguously.
Two distinct record formats observed depending on acquisition mode.

## Encoding A: 6-byte records (non-IMS / simple TOF-MS)

### Status: Partially Decoded

Observed in: PXD058812 (QTOF, native MS, no ion mobility)

Key facts:
- File is a flat array of 6-byte records (no top-level file header)
- Scan boundaries are given by IDX Variant A offsets (u32@0x00)
- Each scan begins with a fixed sentinel record: `00 00 70 ca ff c7`
- Blank/empty scans have exactly 2 records (12 bytes) both being sentinels
- m/z values are NOT stored directly; instead, the TOF time-bin index is stored
  and must be converted using calibration constants from `_extern.inf`

### 6-byte Record Layout

| Bytes | Type | Confirmed | Description |
|-------|------|-----------|-------------|
| 0     | u8   | No        | Flags: 0=normal, 2=? (alternates in ~every 3rd record) |
| 1-3   | u24  | No        | Raw intensity (includes background pedestal ~36864 counts) |
| 4-5   | u16  | **Yes**   | TOF time-bin index (m/z encoded via TOF calibration formula) |

### TOF m/z Decoding

Calibration constants are in `_HEADER.TXT` (`$$ Cal Function 1:`) and `_extern.inf`.
Format `T1` polynomial calibration:

```
t_us = time_bin * (pusher_cycle_us / 65536)
mz = approx (t_us / A)^2     where A = sqrt(m_proton * Lteff / (2 * e * Veff))
```

In practice, use the polynomial coefficients from `Cal Function 1:` for better accuracy.

Validated example (molecular_mass_P15_01.raw, scan 3):
- Lteff=1997.94 mm, Veff=9100 V, pusher=62 us
- time_bin=38747 -> mz=1172.5 Da (protein charge state)
- time_bin=40026 -> mz=1251.2 Da (next charge state)
- sentinel u16=51199 -> mz=2047 Da (~= scan upper limit of 2000 Da)

### Sentinel Record

Every scan starts with `00 00 70 ca ff c7`. This is a fixed magic header.
The u24 field = 28874 (0x70CA) and u16 = 51199 (0xC7FF) in all observed samples.
Purpose unclear (possibly scan metadata marker).

## Encoding B: 8-byte records (IMS mode — SYNAPT G2-Si)

### Status: Partially Decoded

Observed in: PXD066594 (WANG.raw), PXD068881 (CtpA) -- both SYNAPT G2-Si

Key facts:
- File is a flat array of 8-byte records with NO embedded scan headers
- Scan boundaries are given by IDX Variant B offsets (u32@0x16)
- Total: sum(scan record counts) x 8 = file size exactly (confirmed)
- Scan sizes vary (min 636,928 / max 784,640 bytes for WANG.raw) = variable ion detections
- Each 8-byte record represents a single detected ion event with 2D (IMS + TOF) coordinate

### 8-byte Record Layout (IMS mode)

| Bytes | Type | Confirmed | Description |
|-------|------|-----------|-------------|
| 0     | u8   | No        | Flags: nearly always 0; rare values {32, 64, 128, 192} |
| 1-3   | u24  | No        | Raw intensity (24-bit unsigned; ~100k-200k range) |
| 4-7   | u32  | **Yes**   | Compound IMS + TOF coordinate (proprietary packing; see below) |

### IMS Coordinate Encoding

The 4-byte dword at offset 4 encodes BOTH the drift-time axis and the TOF axis in a
proprietary packed format. **This format is not fully decoded without a Waters SDK reference.**

Empirically confirmed facts:
- Records within a scan are sorted ascending by the compound dword
- The upper 16 bits of the dword (bytes 6-7 in LE) span 19632–24027 across **all** scans
  regardless of retention time, corresponding to only ~300–491 Da in a 300–1500 Da scan range
  (i.e., the upper 16 bits are NOT the full TOF bin for the entire m/z range)
- The GCD of the lower 16 bits is 8, suggesting the 3 least-significant bits are always 0
  (possibly used as a sub-field boundary marker)
- Byte[0] flag values {32, 64, 128, 192} are rare (< 0.1% of records); meaning unknown

**Conclusion**: The compound u32 interleaves drift-time bins and TOF bins in a non-trivial
encoding. It cannot be decoded without the Waters MassLynx SDK or an authoritative reference.
The upper-16-bit range constraint alone rules out a simple (TOF_bin << 16) | drift_bin split.

## Encoding C: 8-byte records (non-IMS QTof mode — Xevo G2-XS)

### Status: Decoded

Observed in: PXD075602 (DHPR_11257-1.raw, Xevo G2-XS QTof)

Key facts:
- Same 30-byte IDX Variant B as IMS datasets; DAT offsets at IDX+0x16
- Same 8-byte record size as Encoding B, but structurally different internal layout
- Scan sizes range from 5,776 to 1,019,888 bytes (722–127,486 records per scan)
- Records are sorted ascending by compound coordinate (bytes 4-7)
- Bytes[0-1] are **always 0x0000** (no flags, no drift time — non-IMS instrument)

### 8-byte Record Layout (non-IMS QTof mode)

| Bytes | Type | Confirmed | Description |
|-------|------|-----------|-------------|
| 0-1   | u16  | **Yes**   | Always 0 (reserved / no drift time for non-IMS instruments) |
| 2-3   | u16  | **Yes**   | Intensity (16-bit unsigned; 0–~500 range typical for centroid) |
| 4-5   | u16  | No        | TDC sub-bin position (fine timing within TOF bin cluster; varies per bin) |
| 6-7   | u16  | **Yes**   | TOF bin index → m/z via T1 calibration polynomial |

The compound u32 at bytes 4-7 (read as LE) is `(tof_bin << 16) | sub_bin`, and records
are sorted ascending by this dword, i.e. primarily by `tof_bin`, then by `sub_bin`.

### TOF Bin → m/z Decoding (Encoding C)

Same formula as Encoding A but using calibration constants from `_HEADER.TXT` (`Cal Function N`)
and `_extern.inf`:

```
t_raw_us = tof_bin × (pusher_cycle_us / 65536)
t_cal_us = c0 + c1·t_raw + c2·t_raw² + ...    (T1 polynomial; see 01-header-txt.md)
mz       = (t_cal_us / A_us)²
```

Validated example (DHPR_11257-1.raw, scan 0):
- Lteff=1800 mm, Veff=6328.24 V, pusher=60.3 µs → A_us=1.218477, t_per_bin=0.920105 ns
- tof_bin=13887 → t_raw=12.778 µs → mz≈109.97 Da (first ion in 50–1200 Da scan)
- tof_bin=13901 → mz≈110.19 Da (next detected ion, +0.22 Da)

### Distinguishing Encoding B from C

Both encodings use 8-byte records and IDX Variant B (30-byte stride).
The presence of IMS data can be confirmed by:
- `Apex3DIons.csv` in the `.raw` folder (IMS only)
- IDX +0x04 field: for IMS, the value encodes push count (~32544); for non-IMS Xevo G2-XS it is also non-zero (needs further analysis)
- Bytes[0-1] of every DAT record: always 0x0000 for Encoding C; flags non-zero possible for Encoding B

## Fields Under Investigation

- Encoding A: exact meaning of byte[0] flag values (0 vs 2)
- Encoding A: whether u24@1 is raw TDC counts or scaled intensity
- Encoding B: bit split between drift-time and m/z in the compound u32
- Encoding B: whether byte[0] flag values {32, 64, 128, 192} encode anything useful
- Encoding C: meaning of sub-bin field (bytes 4-5); TDC sub-bin vs other
- Encoding C: IDX +0x04 field interpretation for non-IMS instruments

## Reference Sources

- Empirical hex analysis: `re/src/analysis/inspect.py`
- Calibration: `_extern.inf` (Lteff, Veff, pusher cycle) + `_HEADER.TXT` (Cal Function N)
- Corpus samples:
  - PXD058812/molecular_mass_P15_01.raw (Encoding A, 197 scans, ~1050 rec/scan)
  - PXD058812/MS_fragmentation_P29_01.raw (Encoding A, 426 scans)
  - PXD066594/WANG.raw (Encoding B, 590 scans, 79616–98080 rec/scan)
  - PXD068881/20220517_CtpA_1076_2h_1.raw (Encoding B, 1138 scans)
  - PXD075602/DHPR_11257-1.raw (Encoding C, 1150 scans, 722–127486 rec/scan)
