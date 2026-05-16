# APEXnnnD.BIN / APEXnnnDIONS.CSV

Files produced by the Waters Apex3D peak-detection algorithm.
Present only in IMS-MS acquisitions on SYNAPT G2-Si instruments where
MassLynx post-processing was run (requires Apex3D module installed).

The `nnn` index matches the `_FUNCnnn` function number being processed.
In all corpus examples, only FUNC001 (the survey IMS function) is processed:
`Apex3D.bin` and `Apex3DIons.csv`.

## APEXnnnDIONS.CSV

### Status: Fully Decoded

Human-readable CSV. All columns are self-describing from the header row.

Observed columns (from PXD068881 CtpA):

| Column | Type | Description |
|--------|------|-------------|
| Function | int | Source function number (always 1 in corpus) |
| index | int | Sequential ion index (1-based) |
| m_z | float | Calibrated m/z (Da, 4 decimal places) |
| m_zNoCal | float | m/z before T1 calibration polynomial |
| rt | float | Retention time at peak apex (min) |
| inten | int | Peak apex intensity (TDC counts) |
| area | int | Integrated peak area |
| counts | int | Total raw ion counts within 3D peak |
| errMzPPM | float | m/z error (ppm), if lock mass applied |
| errRT | float | RT error (min) |
| errArea | float | Area error |
| saturation | int | Saturation flag (0 = not saturated) |
| filterReason | str | Rejection code (empty = accepted) |
| m_zNoDT | float | m/z without drift time correction |
| m_zRaw | float | Raw m/z from integer tof_bin (no sub-bin) |
| fIndex | float | accIndex value (= K * sqrt(m_zRaw)) |
| accIndex | float | Accumulated TOF index: K * sqrt(m_zRaw) where K = A_us^-1 × pusher_clock_rate |
| chFWHM | float | Chromatographic peak width (min, FWHM) |
| msFWHM | float | Mass spectral peak width (Da, FWHM) |
| atStartTime ... atStopTime | float | Peak boundary retention times (min) |
| atMsStartMass, atMsStopMass | float | Peak boundary m/z limits (Da) |
| innerElementCount | int | Number of IMS cells contributing to core peak |
| centerSumResponse | int | Summed intensity of core peak cells |
| outerElementCount | int | Number of cells in peak shoulder/tail |
| outerSumResponse | int | Summed intensity of shoulder cells |
| nonZeroElementCount | int | Count of non-zero cells across full peak boundary |
| centerSumCounts | int | Raw TDC counts in core peak |
| outerSumCounts | int | Raw TDC counts in shoulder/tail |

### accIndex field

`accIndex` (= `fIndex`) encodes the centroid TOF flight position in Apex3D-internal units:

```
accIndex = K * sqrt(m_zRaw)
```

where `K = 1 / t_bin_us` (inverse of the per-bin TOF step in microseconds) is derived from
the instrument calibration. For the CtpA dataset K = 4582.48 (units: bins/sqrt(Da)).

This is NOT a raw field from `_FUNCnnn.DAT`. It is a post-processed quantity computed by
Apex3D from the decoded tof_bin using the same calibration constants as the m/z formula.

## APEXnnnD.BIN

### Status: Container structure decoded; per-section data partially characterized

Self-describing multi-section binary container written by the Apex3D executable.
Mirrors the ion data in APEXnnnDIONS.CSV but in binary form plus additional internal state.

### File Header (64 bytes)

| Offset | Type | Value | Description |
|--------|------|-------|-------------|
| 0x00   | u32  | 1     | File format version |
| 0x04   | u32  | 0     | Padding |
| 0x08   | u32  | 64    | Header size in bytes |
| 0x0C   | u32  | 52    | Section descriptor record size |
| 0x10-0x2F | bytes | 0 | Padding |
| 0x30   | u32  | 1     | Unknown (always 1 observed) |
| 0x34   | u32  | 0     | Padding |
| 0x38   | u32  | varies | Byte offset where section data begins |
| 0x3C   | u32  | 0     | Padding |

### Section Descriptor Table

Immediately follows the 64-byte header. Each descriptor is 52 bytes:

```
struct SectionDesc {
    u32  reserved_0;      // first scalar field (semantics unclear; see notes)
    u32  data_offset;     // byte offset of this section's data, relative to data_start
    u32  reserved_8;      // second scalar field (semantics unclear)
    char name[40];        // null-padded ASCII section name
};
```

The number of section descriptors = (data_start - 64) / 52.

The file data area begins at the offset stored in header@0x38. Section data starts at
`header_data_start + section_desc.data_offset`.

Section 0 always has `data_offset = 0` (its data starts immediately at data_start).
Subsequent sections have `data_offset` equal to the cumulative byte extent of previous sections.

### Sections Observed in CtpA (PXD068881)

13 sections:

| Index | Name | Data size | Content |
|-------|------|-----------|---------|
| 0  | AcquisitionParameter | 7230 | Nested parameter table (13 fields; see below) |
| 1  | InstrumentInfo | 834 | Instrument geometry/state (binary; mostly zeros) |
| 2  | FunctionParameter | 550 | Per-function settings (binary) |
| 3  | Calibration | 62 | ASCII: Apex3D command-line args (lock mass, tolerance) |
| 4  | PeakDetectResult | 114 | ASCII: detection parameters used (threshold, function) |
| 5  | ProcessingProgram | 117 | Binary: program version/timestamp |
| 6  | PeakDetectParameter | 659 | Binary f64 array: detection algorithm parameters |
| 7  | DeIsotopeParameter | 116 | Binary f64 array: de-isotoping parameters |
| 8  | BinParameter | 71 | Binary: binning configuration |
| 9  | LmCalibrator | 44 | Binary f64: lock mass calibration coefficients |
| 10 | LockMassStick | 65 | Binary f64: lock mass reference m/z positions |
| 11 | Peak | 388 | Binary: compact peak summary (format not decoded) |
| 12 | PeakEx | 8,502,560 | Binary: extended peak data (format not decoded) |

### Calibration and PeakDetectResult (ASCII sections)

These sections contain the Apex3D command-line invocation that produced the file.
Example (truncated):
```
-lockMassZ1 556.2771 -lockMassToleranceAMU 0.2500
-le3DThresholdCounts 130 -function 1 -bEnableCuda 1
```

### AcquisitionParameter Section

The data at `data_start` is a nested Waters-style parameter table with 13 named fields
describing the acquisition setup (scan time, pusher count, m/z range, etc.).
The inner structure uses a different record format than `_FUNCnnn.STS` and is not
currently fully decoded.

### Peak and PeakEx Binary Data

These sections contain the detected 3D ion peaks in binary form. The file size of PeakEx
(~8.5 MB for CtpA, 1138 scans over 2h) suggests it stores per-scan or per-push IMS-MS
intensity data associated with each detected peak, not just the apex values.

The `APEXnnnDIONS.CSV` file provides all peak apex data in human-readable form. Binary
decoding of Peak/PeakEx is not necessary for reading or writing spectrum data.

## Reference Sources

- Empirical analysis: CtpA (PXD068881)
- CSV format: self-describing column headers
- Binary structure: decoded from hex analysis
