# _FUNCnnn.DAT

Binary spectrum data file. One file per function.
Contains all spectra for that function, stored contiguously.
Three record layouts (encodings) are known. The encoding is a property of the
DAT records, not of the index: Encodings D and E appear behind both the
22-byte (Variant A) and the 30-byte (Variant B) index.

| Encoding | Record size | Index variants | Layout |
|----------|-------------|----------------|--------|
| A | 6 bytes  | A    | u16 ion count, floating-point m/z (4-bit exponent, 24-bit mantissa) |
| D | 8 bytes  | A, B | 16.16 intensity, floating-point m/z (5-bit exponent, 27-bit mantissa) |
| E | 12 bytes | A, B | compressed intensity, Encoding D m/z word, auxiliary word |

Every encoding stores an uncalibrated m/z per record; the `_HEADER.TXT`
T1 polynomial applies to sqrt(m/z). No lock-mass correction is applied by
the reader.

## Choosing the record width

- Variant A index: consecutive DAT offsets divided by the record count give
  6, 8 or 12 bytes per record.
- Variant B index: the record has no record count. The width is judged from
  the data. Under the right width every record's bytes 4-7 are a position
  word with bit 26 set, and m/z never decreases within a scan. Up to 16
  evenly spaced scans are sampled and the first one that fits exactly one of
  8 or 12 bytes (with at least three records) decides. When none decides, the
  reader logs a warning and assumes 8 bytes; a wrong guess then fails at
  decode time with an error rather than producing data.

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

## Encoding D: 8-byte records (either index variant)

### Status: Decoded; m/z checked against physics and lock-mass references

Observed behind the 22-byte index in: PXD081045 (Vion IMS QTof, UNIFI 2.0
export), PXD001123 and PXD009047 (SYNAPT G2, MassLynx 4.1), PXD037102.

Observed behind the 30-byte index in: PXD001175, PXD001471, PXD002393,
PXD005960, PXD035818 (SYNAPT G2-S); PXD066594, PXD068881, PXD079562,
PXD080129 (SYNAPT G2-Si); PXD071342, PXD073126 (SYNAPT XS); PXD045625,
PXD053170, PXD075602, PXD078353 (Xevo G2-XS, all non-lock functions and
some lock functions); PXD069628 (Xevo G3).

Key facts:
- No sentinel records; the first and last records are ordinary profile points
- Zero-intensity records are skipped by the decoder

### 8-byte Record Layout

| Bytes | Type    | Description |
|-------|---------|-------------|
| 0-3   | u32 LE  | Intensity, unsigned 16.16 fixed point (bytes 0-1 are the fraction) |
| 4-7   | u32 LE  | m/z word: 5-bit exponent `E` (bits 27-31), 27-bit mantissa `F` with bit 26 always set |

```
mz_uncal = F * 2^(E - 27)          # F in [2^26, 2^27)
mz       = (T1(sqrt(mz_uncal)))^2
```

A position word without bit 26 set is a decode error.

### Validation (clean-room)

22-byte index:

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

30-byte index:

- In 20 evenly spaced scans of every 8-byte function in the 18 bundles
  above, every record's bytes 4-7 have bit 26 set and m/z never decreases
  within a scan.
- Bytes 0-1 are non-zero in a minority of records (about 1-55% per
  function), with values such as `0xFF00` and `0xE000`, consistent with the
  fraction of a 16.16 intensity.
- Lock mass, median over 20 lock-mass scans, after T1, counting only scans
  where the reference peak's 13C isotope sits one isotope spacing higher
  (`examples/audit_corpus.rs`):

| Bundle | Instrument | Reference | ppm |
|--------|------------|-----------|-----|
| PXD001471 57 | SYNAPT G2-S | Glu-fib 2+ | -17.3 |
| PXD002393 S130426_21 | SYNAPT G2-S | Glu-fib 2+ | -75.6 |
| PXD001175 S121126_06 | SYNAPT G2-S | Glu-fib 2+ | +161.7 |
| PXD068881 CtpA_1076_2h_1 | SYNAPT G2-Si | Leu-Enk | -29.6 |
| PXD080129 186/203/205_nr15 | SYNAPT G2-Si | Leu-Enk | +20.3 each |
| PXD071342 MDE_WT2_DIA | SYNAPT XS | Leu-Enk | +55.0 |
| PXD073126 KMI_sFtsk_2 | SYNAPT XS | Glu-fib 2+ | +37.4 |
| PXD075602 DHPR_11257-1 | Xevo G2-XS | Leu-Enk | +66.5 |
| PXD078353 (two runs) | Xevo G2-XS | Leu-Enk | +48.7, +42.2 |
| PXD069628 HC18_CE, HC20_CE | Xevo G3 | Leu-Enk | -97.6, -107.9 |

  The PXD005960 lock function holds one scan whose dominant ion (m/z 825.1)
  is not a known lock compound. The larger offsets (PXD001175, PXD069628)
  are uniform across the reference peak and its isotope, which points to
  instrument calibration at acquisition time rather than the record model;
  this has not been proven.

## Encoding E: 12-byte records (either index variant)

Original public LCT Premier acquisitions in [MTBLS701](https://www.ebi.ac.uk/metabolights/MTBLS701)
and [MTBLS13770](https://www.ebi.ac.uk/metabolights/MTBLS13770) pair the
22-byte index with 12-byte mass records. Consecutive index offsets divided
by the preceding 24-bit record count establish the width. A separate optical
function in MTBLS701 uses the existing 6-byte encoding and is unchanged.

The lock-mass functions of public Xevo G2-XS bundles PXD045625
(Abu_190520_Sha11, function 3) and PXD053170 (both runs, function 2) pair
the 30-byte index with the same 12-byte layout; their other functions use
Encoding D.

| Offset | Size | Interpretation |
| --- | --- | --- |
| 0 | 4 | Compressed intensity word, little-endian |
| 4 | 4 | Floating-point m/z word, same representation as Encoding D |
| 8 | 4 | Auxiliary word, meaning unresolved |

For intensity word `u`, the measured normalized representation is:

```text
mantissa = u & 0x001fffff
exponent = (u >> 22) & 0x1f
intensity = mantissa * 2^(exponent - 21)
```

Every nonzero intensity in the six LCT mass functions inspected has bit 20
set and bit 21 clear. Higher bits are flags rather than exponent bits; their
meaning is unresolved. The decoder keeps flagged peaks and ignores the
auxiliary word. It rejects unsupported nonzero mantissa patterns. The T1
header calibration applies to sqrt(m/z), as for Encoding D.

Across all 24,484,578 mass records in the two selected LCT acquisitions, raw
positions are ordered within every scan and have the normalized Encoding D
position pattern. Intensity sums excluding bit-28-marked points agree with
the same-file index TIC within 113 ppm. This is a byte-derived consistency
check, not a definition of that flag or external proof of absolute peak
accuracy. No vendor software or vendor-derived expected output was used.
The exact 213-byte `_CHROMS.INF` variant in issue #36 was not present and
remains unresolved.

On the Xevo G2-XS lock-mass functions, every sampled scan decodes without an
intensity-word error and leucine enkephalin lands at +11.0 (PXD045625),
+9.5 and +10.4 ppm (PXD053170) with its 13C isotope one spacing higher. The
intensity scale on these functions has not been checked against a TIC.

This support covers the twelve-byte mass functions. The separate MTBLS701
optical function still uses Encoding A and fails its existing m/z decoder on
some scans. Canonical MTBLS13770 function 2 records also retain the existing
MS2-without-precursor metadata behavior, which does not pass the shared core
conformance check. Neither limitation is corrected by Encoding E.

## Ion mobility (SYNAPT HDMS): not decoded

The reader does not decode ion mobility. Scans from SYNAPT functions are
returned as m/z and intensity only, mzML output carries no mobility array,
and no run declares a mobility array kind.

Known facts:
- SYNAPT functions behind the 30-byte index use Encoding D records. Within
  every sampled scan m/z never decreases, so a stored scan is not split into
  drift-ordered blocks.
- Where drift time is recorded is unresolved. Candidates include the index
  records, the `_PROCnnn` files and per-bundle side files such as
  `mob_cal.csv` (shipped in the PXD080129 bundles).

## Fields Under Investigation

- Encoding A: meaning of the byte 2 low nibble (always zero in the corpus)
- Encoding E: intensity flag bits and the auxiliary word
- Ion mobility: where drift time is stored for SYNAPT acquisitions

## Reference Sources

- Empirical hex analysis: `re/src/analysis/inspect.py`
- Calibration: `_extern.inf` (Lteff, Veff, pusher cycle) + `_HEADER.TXT` (Cal Function N)
- Corpus samples:
  - PXD058812/molecular_mass_P15_01.raw (Encoding A, 197 scans, ~1050 rec/scan)
  - PXD058812/MS_fragmentation_P29_01.raw (Encoding A, 426 scans)
  - PXD066594/WANG.raw (30-byte index, Encoding D, 590 scans)
  - PXD068881/20220517_CtpA_1076_2h_1.raw (30-byte index, Encoding D, 1138 scans)
  - PXD075602/DHPR_11257-1.raw (30-byte index, Encoding D, 1150 scans)
  - PXD053170/20231113_NSE_Sample_High.raw (30-byte index; Encoding D survey,
    Encoding E lock mass)
