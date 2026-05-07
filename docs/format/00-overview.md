# Waters RAW Format - Overview

The Waters MassLynx RAW format is a **directory-based** vendor format used
by Waters LC-MS instruments including the Synapt, Xevo, ACQUITY, and MALDI
HDMS product lines.

Each acquisition produces a `.raw` directory (not a single file) containing
a set of binary and plain-text files that together describe the instrument
method, calibration state, and all acquired spectra.

## Files Present in a Typical .raw Directory

| Filename | Type | Status | Description |
|---|---|---|---|
| `_HEADER.TXT` | ASCII | **Fully known** | Run metadata, calibration polynomials |
| `_FUNCTNS.INF` | Binary | **Fully known** | Function table: one 416-byte record per MS function |
| `_FUNCnnn.IDX` | Binary | **Fully known** | Scan index (DAT offsets, RT, housekeeping) |
| `_FUNCnnn.DAT` | Binary | **Mostly known** | Packed spectrum data (3 encodings; see below) |
| `_FUNCnnn.STS` | Binary | **Fully decoded** | Per-scan instrument statistics (voltages, TIC, push count) |
| `_CHROMS.INF` | Binary | **Fully decoded** | LC channel descriptor table |
| `_CHROnnnn.DAT` | Binary | **Fully decoded** | LC channel time-series data (f32 RT + f32 value) |
| `_extern.inf` | ASCII | **Fully known** | Instrument geometry constants (Lteff, Veff, pusher period) |
| `_INLET.INF` | ASCII text | **Fully known** | ACE inlet method record (LC runs only) |
| `_HISTORY.INF` | Binary | Partially decoded | Waters PT with 0 descriptors; data opaque |
| `_PROCnnn.DAT/IDX/STS` | Binary | Partially decoded | Post-processed IMS-MS peak data (IMS runs only) |
| `APEXnnnD.BIN` | Binary | Undocumented | Apex3D IMS peak-detection output (IMS only) |
| `APEXnnnDIONS.CSV` | CSV | Undocumented | Apex3D ion list (IMS only) |

Files without a number suffix appear once per `.raw` directory. Files with
`nnn` are numbered 001–099, one per MS function.

## Function Concept

A "function" in Waters terminology is a discrete acquisition channel. A
typical experiment structure:

| Experiment type | Functions |
|-----------------|-----------|
| MS survey only | Function 1 = MS1 |
| DDA (auto-MS/MS) | Function 1 = survey, Functions 2–N = triggered MS/MS |
| IMS-MS (HDMS) | Function 1 = IMS-MS, Function 2 = reference/lock-mass |
| Lock-mass reference | Last function = calibrant channel |

An MRM experiment (triple-quadrupole) would have one function per
precursor/product pair. MRM data is rare in public repositories and
this format variant has not yet been observed in corpus data.

## DAT Encoding Variants

Three distinct record encodings have been observed in `_FUNCnnn.DAT`:

| Encoding | Record size | IDX variant | Instruments | Description |
|----------|-------------|-------------|-------------|-------------|
| A | 6 bytes | Variant A (22-byte IDX) | Older QTOF (Q-TOF Ultima) | flags(u8), intensity(u24), TOF bin(u16) |
| B | 8 bytes | Variant B (30-byte IDX) | SYNAPT G2-Si IMS | flags(u8), intensity(u24), proprietary drift+TOF compound u32 |
| C | 8 bytes | Variant B (30-byte IDX) | Xevo G2-XS QTof | zero(u16), intensity(u16), sub-bin(u16), TOF bin(u16) |

Encoding B (IMS) uses a proprietary compound coordinate in bytes 4–7 that
encodes both drift time and TOF bin in an undecoded packing. Encodings A
and C are fully decodable to m/z using the T1 calibration polynomial.

## IDX Variants

| Variant | Record size | DAT offset field | Observed in |
|---------|-------------|-----------------|-------------|
| A | 22 bytes | u32@0x00 | Older non-IMS QTOF |
| B | 30 bytes | u32@0x16 | SYNAPT G2-Si (IMS and non-IMS), Xevo G2-XS |

Variant B is used by both IMS and non-IMS Xevo/SYNAPT G2-generation
instruments. IDX stride alone does not distinguish IMS from non-IMS;
presence of `APEXnnnD.BIN` or `APEXnnnDIONS.CSV` is the reliable IMS indicator.

## m/z Decoding Summary

For Encodings A and C (decodable):

```
# Common to both:
A_us   = sqrt(m_proton * Lteff_m / (2 * e * Veff)) * 1e6  # from _extern.inf
mz     = (t_cal_us / A_us)^2
t_cal  = c0 + c1*t_raw + c2*t_raw^2 + ... + ck*t_raw^k  # T1 polynomial, _HEADER.TXT

# Encoding A (6-byte, non-IMS QTOF):
#   First record of each scan is a zero-intensity sentinel;
#   sentinel.tof_bin = max TOF bin corresponding to mz_high.
t_bin_us   = A_us * sqrt(mz_high) / sentinel_tof_bin  # bin width in microseconds
t_raw_us   = tof_bin * t_bin_us

# Encoding C (8-byte, Xevo G2-XS):
#   First record = sentinel at mz_low_bin, last = sentinel at mz_high_bin.
t_low_us   = A_us * sqrt(mz_low)
t_high_us  = A_us * sqrt(mz_high)
t_bin_us   = (t_high_us - t_low_us) / (mz_high_bin - mz_low_bin)
frac_bin   = (tof_bin - mz_low_bin) + sub_bin / 65536
t_raw_us   = t_low_us + frac_bin * t_bin_us
```

where `m_proton = 1.6726e-27 kg`, `e = 1.6022e-19 C`,
`Lteff_m = Lteff_mm / 1000`, and `mz_low`/`mz_high` come from `_FUNCTNS.INF`.

## Waters Parameter Table Format

Several binary files (`_CHROMS.INF`, `_FUNCnnn.STS`, `_CHROnnnn.DAT`) share
a common "parameter table" structure:

```
[32-byte preamble]
  u16@0 = data_offset  (= 32 + n_desc * 48)
  u16@2 = version (always 1)
  u16@4 = record_size
  u16@6 = n_desc
[n_desc * 48-byte descriptor records, starting at 0x20]
  u16@0 = channel sequence number
  u16@2 = encoding type (0=u8, 1=i16, 2=u32, 3=f32)
  u16@4 = byte offset in data record
  bytes[6:48] = null-padded ASCII channel name
[n_records * record_size bytes of data]
```

_CHROMS.INF uses a 128-byte header + 85-byte records (different stride).

## Known Instrument Generations

| Instrument | Notes |
|---|---|
| Waters SYNAPT G2-Si | IMS + MS; IDX Variant B; DAT Encoding B (IMS) |
| Waters Xevo G2-XS QTof | No IMS; IDX Variant B; DAT Encoding C |
| Waters Q-TOF Ultima | No IMS; IDX Variant A; DAT Encoding A |

## Corpus

| Accession | Instrument | Notes |
|-----------|-----------|-------|
| PXD058812 | Q-TOF (non-IMS) | 3 small files, Encoding A, 197–426 scans |
| PXD066594 | SYNAPT G2-Si IMS | WANG.raw, 590 scans, large IMS data |
| PXD068881 | SYNAPT G2-Si IMS | CtpA LC-MS, 1138 scans, has CHROMS.INF |
| PXD075602 | Xevo G2-XS QTof | DHPR LC-MS, 3 functions, Encoding C |

## See Also

- [01 - _HEADER.TXT](01-header-txt.md)
- [02 - _FUNCTNS.INF](02-functns-inf.md)
- [03 - _FUNCnnn.IDX](03-func-idx.md)
- [04 - _FUNCnnn.DAT](04-func-dat.md)
- [05 - _CHROMS.INF](05-chroms-inf.md)
- [06 - _extern.inf](06-extern-inf.md)
- [07 - _FUNCnnn.STS](07-func-sts.md)
- [08 - _CHROnnnn.DAT](08-chro-dat.md)
- [09 - _PROCnnn files](09-proc-files.md)
- [10 - _INLET.INF / _HISTORY.INF](10-aux-files.md)
