# _FUNCnnn.IDX

Binary scan index file. One file per function (e.g. _FUNC001.IDX,
_FUNC002.IDX). Provides random access into the paired .DAT file.

## Status: Partially Decoded (Phase 2)

Two record sizes observed, corresponding to two DAT encoding schemes.

## Variant A: 22-byte record (non-IMS / simple TOF-MS)

Observed in: PXD058812 (Waters QTOF, native MS, no ion mobility)

Key facts:
- File size = N x 22 bytes (exact, no header)
- u32@0x00 = DAT byte offset -- confirmed
- Scan 0-2 are often zero-data "blank" scans (12 bytes each in DAT)

| Offset | Type | Confirmed | Description |
|--------|------|-----------|-------------|
| 0x00   | u32  | **Yes**   | Byte offset into .DAT file |
| 0x04   | u32  | No        | Function/type code (0x18000002 for empty, varies for data scans) |
| 0x08   | f32  | No        | Base peak intensity or TIC (0 for blank scans) |
| 0x0C   | f32  | **Yes**   | Retention time (minutes) |
| 0x10   | u16  | No        | Centroid peak count (0 for blank, ~17-137 per scan) |
| 0x12   | u16  | No        | Unknown |
| 0x14   | u16  | No        | Unknown |

Validated: 22 x 197 = 4334 bytes (molecular_mass_P15_01.raw), 22 x 426 = 9372 bytes (MS_fragmentation_P29_01.raw)

## Variant B: 30-byte record (IMS / HDMS and non-IMS QTof)

Observed in:
- PXD066594 (WANG.raw, SYNAPT G2-Si, IMS)
- PXD068881 (CtpA, SYNAPT G2-Si, IMS)
- PXD075602 (DHPR_11257-1.raw, Xevo G2-XS QTof, **non-IMS**)

Key facts:
- File size = N x 30 bytes (exact, no header)
- DAT byte offset stored at +0x16 (NOT +0x00) -- confirmed for all three datasets
- Scan sizes vary depending on ion density
- Total records check: sum(scan_sizes) / 8 = DAT_size / 8 exactly (flat 8-byte record array)
- The Xevo G2-XS QTof (non-IMS) uses this variant despite having no drift dimension;
  the IDX stride is 30 bytes and DAT records are 8 bytes (see `_FUNCnnn.DAT` Encoding C)

| Offset | Type | Confirmed | Description |
|--------|------|-----------|-------------|
| 0x00   | u32  | No        | Flags (always 0 in tested datasets) |
| 0x04   | u32  | Partial   | Hardware tick counter (scan duration); see note below |
| 0x08   | u32  | Partial   | Hardware counter, scan-varying; see note below |
| 0x0C   | f32  | **Yes**   | Retention time (minutes) |
| 0x10   | f32  | No        | Varies; could be base peak m/z |
| 0x14   | u16  | No        | Small value, purpose unknown |
| 0x16   | u32  | **Yes**   | Byte offset into .DAT file |
| 0x1A   | u32  | No        | Always 0 in tested datasets |

Validated: sum of (IDX[i+1].offset - IDX[i].offset) for all i = DAT file size exactly.

### Field +0x04: Hardware tick counter

Observed range: approximately 402.6M-402.8M across all corpus instruments.
Varies by +/-50K between consecutive scans (jitter in scan execution time).

Hypothesis: total 800-MHz reference-clock ticks elapsed during the scan.
Predicted value = n_pushes x (pusher_period_us x 800):

| Dataset | n_pushes | IDX+0x04 / n_pushes | Implied period at 800 MHz |
|---------|----------|---------------------|--------------------------|
| DHPR    | 8299     | 48520               | 60.6 us                  |
| WANG    | 14389    | 27990               | 35.0 us                  |
| CtpA    | 4317     | 93299               | 116.6 us                 |

DHPR result (60.6 us) is consistent with the FUNCTNS.INF-derived 60.25 us pusher period.
WANG result (35.0 us) corrects the previous 69 us estimate from tof_depth alone.
CtpA (116.6 us) is anomalously long; may reflect a different acquisition mode.

The 800 MHz figure is inferred; the actual clock rate is not confirmed from
available data.

### Field +0x08: Scan counter (purpose unclear)

Observed range: approximately 1.17B-1.29B across corpus. Fluctuates by up to
7% from first to last scan of a run. The variation pattern across scans is
consistent with a TIC chromatogram (rises and falls with elution) but the
absolute value does not match the sum of intensity values from the DAT records
by any simple factor. May encode raw TDC hit counts from the detector hardware
before per-scan intensity processing.

## Distinguishing Variant A from B

- Check file_size mod 22 == 0 (Variant A) or mod 30 == 0 (Variant B)
- Both should be mutually exclusive in practice
- Variant B is used by both IMS (SYNAPT) and non-IMS (Xevo G2-XS) instruments
- Presence of Apex3DIons.csv strongly implies IMS mode even if IDX is Variant B

## Fields Under Investigation

- Variant A: encoding of +0x04 function/type field
- Variant B: +0x04 clock-rate confirmation (800 MHz assumed); +0x08 TDC/TIC encoding
- Whether a 32-byte variant exists for other instrument generations

## Reference Sources

- Empirical hex analysis: `re/src/analysis/inspect.py records`
- Corpus samples:
  - PXD066594/WANG.raw (Variant B, SYNAPT G2-Si IMS, 590 scans)
  - PXD068881/20220517_CtpA_1076_2h_1.raw (Variant B, SYNAPT G2-Si IMS, 1138 scans)
  - PXD058812/molecular_mass_P15_01.raw (Variant A, QTOF non-IMS, 197 scans)
  - PXD058812/MS_fragmentation_P29_01.raw (Variant A, QTOF non-IMS, 426 scans)
  - PXD075602/DHPR_11257-1.raw (Variant B, Xevo G2-XS QTof non-IMS, 1150 scans)
