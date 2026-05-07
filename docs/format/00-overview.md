# Waters RAW Format - Overview

The Waters MassLynx RAW format is a **directory-based** vendor format used
by Waters LC-MS instruments including the Synapt, Xevo, ACQUITY, and MALDI
HDMS product lines.

Each acquisition produces a `.raw` directory (not a single file) containing
a set of binary and plain-text files that together describe the instrument
method, calibration state, and all acquired spectra.

## Files Present in a Typical .raw Directory

| Filename | Type | Description |
|---|---|---|
| `HEADER.TXT` | ASCII | Run metadata: instrument, operator, date, sample description |
| `_FUNCTNS.INF` | Binary | Function table: one record per acquisition function |
| `_FUNCnnn.DAT` | Binary | Packed spectrum data for function n (001-099) |
| `_FUNCnnn.IDX` | Binary | Index records for function n (offset, length, RT per scan) |
| `_CHROMS.INF` | Binary | Chromatogram metadata and TIC/BPI data |
| `_INLET.INF` | Binary | LC inlet / pump conditions |
| `_MASSLYNX.PRO` | Binary | Acquisition method parameters |
| `_CALDATA.INF` | Binary | Mass calibration data (optional) |
| `_MS_DATA.INF` | Binary | Additional MS acquisition settings (optional) |

## Function Concept

A "function" in Waters terminology is a discrete acquisition channel. A
typical DDA experiment has:

- Function 1: MS1 survey scan (continuum or centroid)
- Functions 2-N: MS2 fragmentation scans triggered by IDA

An MRM experiment may have one function per precursor/product pair. IMS
experiments add an additional mobility dimension within a function.

## Known Instrument Generations

| CV Accession | Instrument |
|---|---|
| MS:1001790 | Waters SYNAPT G2-Si |
| MS:1001789 | Waters SYNAPT G2 |
| MS:1001788 | Waters SYNAPT HDMS |
| MS:1002278 | Waters Xevo G2-XS QTof |
| MS:1002279 | Waters Xevo G2 QTof |
| MS:1001785 | Waters ACQUITY UPLC |
| MS:1001782 | Waters Synapt MS |

## See Also

- [01 - HEADER.TXT](01-header-txt.md)
- [02 - _FUNCTNS.INF](02-functns-inf.md)
- [03 - _FUNCnnn.IDX](03-func-idx.md)
- [04 - _FUNCnnn.DAT](04-func-dat.md)
- [05 - _CHROMS.INF](05-chroms-inf.md)
