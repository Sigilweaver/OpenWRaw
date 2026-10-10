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
| `_FUNCnnn.DAT` | Binary | Partially decoded | Packed spectrum data (3 encodings; see below). Ion mobility not decoded |
| `_FUNCnnn.STS` | Binary | **Fully decoded** | Per-scan instrument statistics (voltages, TIC, push count) |
| `_CHROMS.INF` | Binary | **Fully decoded** | LC channel descriptor table |
| `_CHROnnnn.DAT` | Binary | **Fully decoded** | LC channel time-series data (f32 RT + f32 value) |
| `_extern.inf` | ASCII | **Fully known** | Instrument geometry constants (Lteff, Veff, pusher period) |
| `_INLET.INF` | ASCII text | **Fully known** | ACE inlet method record (LC runs only) |
| `_HISTORY.INF` | Binary | Partially decoded | Waters PT with 0 descriptors; data opaque |
| `_PROCnnn.DAT/IDX/STS` | Binary | Partially decoded | Post-processed IMS-MS peak data (IMS runs only) |
| `APEXnnnD.BIN` | Binary | Container decoded | Multi-section binary; ASCII command-line params + binary peak data (PeakEx not decoded) |
| `APEXnnnDIONS.CSV` | CSV | **Fully known** | Apex3D ion list: m/z, RT, intensity, drift time per detected 3D peak |

Files without a number suffix appear once per `.raw` directory. Files with
`nnn` are numbered 001-099, one per MS function.

## Function Concept

A "function" in Waters terminology is a discrete acquisition channel. A
typical experiment structure:

| Experiment type | Functions |
|-----------------|-----------|
| MS survey only | Function 1 = MS1 |
| MSe / HDMSe (broadband) | Function 1 = low-energy, Function 2 = high-energy (both `TOF PARENT FUNCTION`, `Precursor Selection: Everything` - no discrete precursor) |
| Targeted MS/MS | Function 1 = `TOF MSMS FUNCTION` with a fixed `Set Mass` (single precursor per acquisition, not survey-triggered) |
| IMS-MS (HDMS) | Function 1 = IMS-MS, Function 2 = reference/lock-mass |
| Lock-mass reference | Last function = calibrant channel |

An MRM experiment (triple-quadrupole) would have one function per
precursor/product pair. MRM data is rare in public repositories and
this format variant has not yet been observed in corpus data.

The corpus's one targeted-MS/MS sample so far (PXD035818, single
"TOF MSMS FUNCTION" per file) is a fixed-precursor direct-infusion
acquisition, not a classic multi-function survey-then-trigger DDA method;
no real sample of the latter has been found yet either (Sigilweaver/OpenWRaw#13).

## DAT Encoding Variants

Three record encodings are distinguished in `_FUNCnnn.DAT`. The encoding is a
property of the DAT records; Encodings D and E appear behind both index
variants.

| Encoding | Record size | IDX variant | Instruments | Description |
|----------|-------------|-------------|-------------|-------------|
| A | 6 bytes  | A (22-byte) | Older QTOF, Q-Tof Premier class | count(u16), m/z word (exponent nibble + u24 mantissa) |
| D | 8 bytes  | A or B | Vion (UNIFI export), SYNAPT G2/G2-S/G2-Si/XS, Xevo G2-XS, Xevo G3 | intensity (u32 16.16), m/z word (5-bit exponent + 27-bit mantissa) |
| E | 12 bytes | A or B | LCT Premier; some Xevo G2-XS lock-mass functions | compressed intensity, Encoding D m/z word, auxiliary word |

All three store m/z directly and are checked against lock-mass references
(see `_FUNCnnn.DAT`).

## IDX Variants

| Variant | Record size | DAT offset field | Observed in |
|---------|-------------|-----------------|-------------|
| A | 22 bytes | u32@0x00 | Older QTOF, Vion (UNIFI export), SYNAPT G2, LCT Premier |
| B | 30 bytes | u64@0x16 | SYNAPT G2-S/G2-Si/XS, Xevo G2-XS, Xevo G3 |

Variant A records carry a record count, so the DAT record width follows from
consecutive offsets. Variant B records do not; the reader judges the width
from the position words of sampled scans. The index variant does not
indicate an ion mobility acquisition.

## m/z Decoding Summary

Every encoding stores an uncalibrated m/z as a floating-point word; only the
T1 polynomial from `_HEADER.TXT` applies, to sqrt(m/z). No lock-mass
correction is applied.

```
mz_uncal = mantissa * 2^(exponent - mantissa_bits)   # 24 bits (A) or 27 bits (D, E)
mz       = (T1(sqrt(mz_uncal)))^2
T1(x)    = c0 + c1*x + c2*x^2 + ... + ck*x^k          # "Cal Function N", _HEADER.TXT
```

## Ion Mobility

Ion mobility is not decoded. SYNAPT scans are returned as m/z and intensity
only; where drift time is stored is unresolved (see `_FUNCnnn.DAT`).

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

_CHROMS.INF uses the same 32-byte preamble and two 48-byte field
descriptors (128 bytes total), followed by 85-byte channel records.
Its descriptor count is not a count of records to skip after the header.

## Known Instrument Generations

| Instrument | Notes |
|---|---|
| Waters SYNAPT G2-S / G2-Si / XS | IMS-capable; IDX Variant B; DAT Encoding D (mobility not decoded) |
| Waters Xevo G2-XS QTof | No IMS; IDX Variant B; DAT Encoding D (some lock-mass functions Encoding E) |
| Waters Xevo G3 QTof | No IMS; IDX Variant B; DAT Encoding D |
| Waters Q-TOF Ultima | No IMS; IDX Variant A; DAT Encoding A |
| Waters Vion IMS QTof (UNIFI export) | IDX Variant A; DAT Encoding D |

## Corpus

| Accession | Instrument | Notes |
|-----------|-----------|-------|
| PXD058812 | Q-TOF (non-IMS) | 3 small files, Encoding A, 197-426 scans |
| PXD066594 | SYNAPT G2-Si | WANG.raw, 590 scans, Encoding D |
| PXD068881 | SYNAPT G2-Si | CtpA LC-MS, 1138 scans, Encoding D, has CHROMS.INF |
| PXD075602 | Xevo G2-XS QTof | DHPR LC-MS, 3 functions, Encoding D |
| PXD035818 | SYNAPT G2-S | 17122018_TNFA_PEPTIDE_GSHH_MSMS_884.raw, targeted MS/MS (`TOF MSMS FUNCTION`, Set Mass 884.9), IDX Variant B, Encoding D |

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
