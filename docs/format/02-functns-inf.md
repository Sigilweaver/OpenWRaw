# _FUNCTNS.INF

Binary function descriptor table. Present in every .raw directory.
One record per acquisition function (survey MS, IMS, MS/MS, chromatogram, etc.).

## Status: Partially Decoded (Phase 2)

Empirically confirmed from two SYNAPT G2-Si datasets:
- **PXD066594/WANG.raw** (1 function, 300–1500 m/z, 0.014 min scan time)
- **PXD068881/20220517_CtpA_1076_2h_1.raw** (3 functions, 100–2000 m/z)

## Record Layout (416 bytes per function)

| Offset | Type  | Confirmed | Description |
|--------|-------|-----------|-------------|
| 0x000  | ?     | No        | Unknown (differs per dataset) |
| 0x004  | ?     | No        | Unknown (slightly varies) |
| 0x008  | u32?  | No        | Same across both (0x00003c65); possibly IMS bin count |
| 0x010  | u16?  | No        | Varies slightly per function (16672 / 16704) |
| 0x014  | u32   | No        | Always 0x01000000; possibly scan count or calibration flag |
| 0x01C  | f32   | **Yes**   | Scan time (minutes) — 0.014 min ≈ 0.84 s |
| 0x020  | f32   | No        | Multiplier/sensitivity (1.0 vs 0.3 across datasets) |
| 0x0A0  | f32   | **Yes**   | Mass range LOW (m/z) — 300.0, 100.0 |
| 0x120  | f32   | **Yes**   | Mass range HIGH (m/z) — 1500.0, 2000.0 |

Note: All non-listed offsets in range 0–415 are zero.

## Key Facts

- File size = N × 416 bytes, where N = number of acquisition functions
- No file header prefix (first byte is first function record)
- Functions are numbered 1..N and correspond to `_FUNC001.DAT/.IDX/.STS`, etc.
- SYNAPT G2-Si uses up to 3 functions (survey + IMS + lockmass or processed)

## Fields Under Investigation

- Function type code (MS scan vs IMS vs MRM vs diode array)
- Polarity (positive / negative)
- IMS parameters: travelling wave voltage, drift time bins
- Exact meaning of bytes 0x000–0x00F

## Reference Sources

- ProteoWizard `MassLynxRawReader` (Windows-only DLL wrapper — uses SDK)
- Empirical hex analysis: `analysis/src/analysis/inspect.py`
