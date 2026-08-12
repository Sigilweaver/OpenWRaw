# _extern.inf

ASCII key-value instrument parameter file. Always present. Records the
instrument geometry constants and per-function acquisition method parameters
needed to decode TOF time bins into m/z values.

## Status: Fully Known

_extern.inf is plain text and human-readable. It is the primary source
for the calibration constants `Lteff`, `Veff`, and `PusherInterval` that
appear in every m/z decoding formula documented in this project.

## Encoding

Windows-1252. Contains characters such as `µ` (0xB5) and `°` (0xB0).
Line endings are CRLF on Windows-generated files.

## File Structure

The file is divided into labelled sections. Sections begin with a line
ending in `:` (no leading whitespace). Fields within a section are
indented with one or more spaces:

```
Parameters for <method_file_path>
Created by <masslynx_version>
<Section Heading>:
  Field Name                    value
  ...
Function Parameters - Function 1 - <FUNCTION_TYPE>
  Field Name                    value
  ...
Function Parameters - Function 2 - <FUNCTION_TYPE>
  ...
```

There is no strict alignment requirement; fields are whitespace-separated
with the field name (possibly multi-word, possibly containing parentheses
and units) on the left and the value on the right.

## Key Fields for m/z Decoding

These fields are required to convert a stored TOF bin index to m/z.
All are in the `Instrument Configuration:` section unless noted.

| Field Name | Units | Description |
|------------|-------|-------------|
| `Lteff` | mm | Effective TOF flight path length |
| `Veff`  | V  | Effective accelerating voltage |
| `PusherInterval` | µs | Actual pusher cycle period (authoritative value) |

`PusherInterval` is the field used to convert a bin index to a raw flight
time:

```
t_raw_us = tof_bin * (PusherInterval / 65536)
```

Several other field names encode the same physical quantity but appear in
different instrument generations or contexts:

| Also observed | Notes |
|---------------|-------|
| `Pusher Cycle Time (µs)` | In older Instrument Parameters sections; may read `Automatic` (ignore if so) |
| `ADC Pusher Frequency (µs)` | Per-function override; use if present for that function |

When `Pusher Cycle Time (µs)` reads `Automatic`, use `PusherInterval` from
the `Instrument Configuration:` section.

## Instrument Configuration Section

Present in all observed datasets. Key fields:

| Field | Observed values | Description |
|-------|----------------|-------------|
| `Lteff` | 1800.0-1997.94 mm | Effective flight path length |
| `Veff` | 6328.24-9100.0 V | Effective accelerating voltage |
| `Resolution` | 4000-22000 | Nominal resolving power |
| `Min Points in Peak` | 2 | Minimum peak points for centroiding |
| `Acquisition Device` | `WatersADC` | ADC type (always seen) |
| `PusherInterval` | 60.25-88.0 µs | Authoritative pusher cycle time |
| `PusherOffset` | 0.0-0.25 µs | Pusher timing offset |
| `EDC Delay Coefficient` | 1.33-1.40 | Enhanced Duty Cycle delay constant |
| `EDC Delay Offset` | 0.40 µs | EDC delay offset |

IMS-capable instruments (SYNAPT G2-Si) additionally have:

| Field | Description |
|-------|-------------|
| `ADC Pushes Per IMS Increment` | Pushes accumulated per mobility bin (always 1) |
| `IMS Wave Velocity (m/s)` | Travelling wave velocity |
| `IMS Wave Height (V)` | Travelling wave amplitude |
| `Trap Wave Velocity (m/s)` | Trap region wave velocity |
| `Transfer Wave Velocity (m/s)` | Transfer region wave velocity |
| `Drift Time Bins` | Number of IMS drift bins (0 = auto) |

## Instrument Parameters Section

Contains per-run electrode voltages and source conditions. Key fields:

| Field | Units | Description |
|-------|-------|-------------|
| `Polarity` | - | `ES+` or `ES-` (electrospray polarity) |
| `Capillary (kV)` | kV | Spray capillary voltage |
| `Sampling Cone` | V | Cone voltage |
| `Source Temperature (°C)` | °C | Desolvation source temp |
| `Desolvation Temperature (°C)` | °C | Desolvation gas temp |
| `Pusher` | V | Pusher electrode voltage |
| `Puller` | V | Puller electrode voltage |
| `Flight Tube (kV)` | kV | Main flight tube voltage |
| `Reflectron (kV)` | kV | Reflectron voltage |
| `Collision Energy` | eV | Default collision energy |
| `Detector` | V | Detector voltage (MCP bias) |

## Function Parameters Sections

One section per MS function. Format:
`Function Parameters - Function N - <TYPE>`

where `TYPE` is one of:
- `TOF MS FUNCTION` - standard MS1 survey scan
- `TOF PARENT FUNCTION` - MSe/HDMSe low- or high-energy scan (broadband,
  non-selective fragmentation - `[INCLUDE]`'s `Precursor Selection` reads
  `Everything`, so there is no discrete precursor to report; see
  `docs/format/00-overview.md`'s Function Concept table)
- `TOF MSMS FUNCTION` - targeted MS/MS (fixed precursor, classic DDA-style
  acquisition). Carries a `Set Mass` field (see below).
- `TOF DAUGHTER FUNCTION` - DDA fragment scan, same `Set Mass` semantics as
  `TOF MSMS FUNCTION`
- `TOF PRODUCT FUNCTION` - MS/MS fragment scan
- `TOF CALIBRATION FUNCTION` - lock-mass reference channel

Key per-function fields:

| Field | Units | Description |
|-------|-------|-------------|
| `Start Mass` | Da | Acquisition m/z lower limit |
| `End Mass` | Da | Acquisition m/z upper limit |
| `Start Time (mins)` | min | Acquisition start retention time |
| `End Time (mins)` | min | Acquisition end retention time |
| `Scan Time (sec)` | s | Duration of one scan |
| `Interscan Time (sec)` | s | Dead time between scans |
| `Data Format` | - | `Continuum` or `Centroid` |
| `Analyser` | - | `Resolution Mode` or `Sensitivity Mode` |
| `ADC Sample Frequency (GHz)` | GHz | ADC sampling rate |
| `ADC Pusher Frequency (µs)` | µs | Per-function pusher cycle override (if set) |
| `Calibration` | - | Calibration mode: `Dynamic 2` = lock-mass, `Static` = fixed |
| `Set Mass` | Da | Targeted precursor m/z. Only present on `TOF MSMS FUNCTION` / `TOF DAUGHTER FUNCTION` sections - confirmed against real targeted-MS/MS corpus samples (Sigilweaver/OpenWRaw#13), absent from every `TOF PARENT FUNCTION` (MSe) section checked |

No charge state or isolation-width field has been found on any function
type in any corpus sample; `Set Mass` (target m/z) is the only precursor
field `_extern.inf` exposes. Per-scan collision energy for MS/MS-classified
functions (both targeted and MSe) comes from a separate file - see
[07 - _FUNCnnn.STS](07-func-sts.md)'s "Collision Energy" channel.

`ExternFunction::start_mass_da` and `end_mass_da` preserve the text fields
decoded from this file, but spectrum decoding continues to use
`_FUNCTNS.INF`'s `FunctionInfo::mz_low` / `mz_high`. The corpus and current
output model do not establish whether the two pairs are aliases, bounds with
different meanings, or which source should take precedence when they differ,
so the extern values are not substituted into the decode path
(Sigilweaver/OpenWRaw#24).

## Version Line

`Created by <version_string>` gives the MassLynx version that wrote the file:

| Observed | Instrument |
|----------|------------|
| `4.1 SCN 965` | SYNAPT G2-Si (PXD068881) |
| `4.2 SCN1003` | Q-TOF non-IMS (PXD058812) |
| `4.2 SCN1003` | SYNAPT G2-Si (PXD066594) |
| `MassLynx v4.2 SCN966` | Xevo G2-XS QTof (PXD075602) |

SCN = software change notice (patch level within the major release).

## Corpus Cross-Reference

| Corpus | Instrument | Lteff (mm) | Veff (V) | Pusher (µs) |
|--------|------------|-----------|---------|------------|
| PXD058812/molecular_mass_P15_01.raw | Q-TOF non-IMS | 1997.85 | 9100.0 | 88 (Cycle Time) → bin→t: 88/65536 |
| PXD066594/WANG.raw | SYNAPT G2-Si IMS | 1800.0 | 7202.70 | 69.0 (PusherInterval) |
| PXD068881/CtpA.raw | SYNAPT G2-Si IMS | 1800.0 | 7198.65 | 69.0 (PusherInterval) |
| PXD075602/DHPR_11257-1.raw | Xevo G2-XS QTof | 1800.0 | 6328.24 | 60.25 (PusherInterval) |
| PXD035818/17122018_TNFA_PEPTIDE_GSHH_MSMS_884.raw | SYNAPT G2-S | 1800.0 | 7189.15 | 69.0 (PusherInterval) |

The PXD035818 sample is the first targeted MS/MS (`TOF MSMS FUNCTION`) file
in the corpus (Sigilweaver/OpenWRaw#13); Function 1's `Set Mass` reads
`884.9`.

## Reference Sources

- Corpus files: all `_extern.inf` files in PXD058812, PXD066594, PXD068881, PXD075602, PXD035818
- Used by: `_HEADER.TXT` calibration polynomial, `_FUNCnnn.DAT` Encoding A/C m/z decode, `mzml::precursor_info_for` (`target_mz`)
