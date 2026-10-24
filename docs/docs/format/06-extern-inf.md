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

## Flight-Time Geometry Fields

No `_FUNCnnn.DAT` encoding stores a TOF bin; every encoding stores m/z
directly and only the `_HEADER.TXT` polynomial is applied. These fields
describe the flight-time geometry. They are used to check the ADC sample
spacing of Encoding D records (`t = A_us * sqrt(m/z)`), not to decode m/z.
All are in the `Instrument Configuration:` section unless noted.

| Field Name | Units | Description |
|------------|-------|-------------|
| `Lteff` | mm | Effective TOF flight path length |
| `Veff`  | V  | Effective accelerating voltage |
| `PusherInterval` | µs | Actual pusher cycle period (authoritative value) |

`PusherInterval` is the pusher cycle period. Several other field names encode the same physical quantity but appear in
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
- `TOF MS FUNCTION` - MS1 scan; some MSe acquisitions also label their
  high-energy function this way (see "MS level" below)
- `TOF PARENT FUNCTION` - MSe/HDMSe low- or high-energy scan (broadband,
  non-selective fragmentation - `[INCLUDE]`'s `Precursor Selection` reads
  `Everything`, so there is no discrete precursor to report; see
  `docs/format/00-overview.md`'s Function Concept table)
- `TOF MSMS FUNCTION` - MS/MS. Targeted (one fixed precursor) or
  data-dependent. Carries a `Set Mass` field (see below).
- `TOF SURVEY FUNCTION`, `MOBILITY SURVEY FUNCTION`, `MOBILITY FAST DDA
  FUNCTION` - data-dependent (DDA) acquisition: function 1 is the MS1
  survey, the following functions are the MS/MS slots, each with
  `Set Mass 0.0`
- `TOF DAUGHTER FUNCTION` - DDA fragment scan, same `Set Mass` semantics as
  `TOF MSMS FUNCTION`
- `TOF PRODUCT FUNCTION` - MS/MS fragment scan
- `TOF CALIBRATION FUNCTION` - lock-mass reference channel

Key per-function fields:

| Field | Units | Description |
|-------|-------|-------------|
| `Start Mass` | Da | Acquisition m/z lower limit. Not parsed into a struct field - see note below |
| `End Mass` | Da | Acquisition m/z upper limit (MSMS/DAUGHTER sections instead use `MSMS End Mass`, which is not recognised by the parser either). Not parsed into a struct field - see note below |
| `Start Time (mins)` | min | Acquisition start retention time |
| `End Time (mins)` | min | Acquisition end retention time |
| `Scan Time (sec)` | s | Duration of one scan |
| `Interscan Time (sec)` | s | Dead time between scans |
| `Data Format` | - | `Continuum` or `Centroid`. Not parsed. Every function in the corpus that declares it says `Continuum`, including lock-mass functions whose DAT holds centroids (see `_FUNCnnn.DAT`, "Profile or centroid") |
| `Analyser` | - | `Resolution Mode` or `Sensitivity Mode` |
| `ADC Sample Frequency (GHz)` | GHz | ADC sampling rate |
| `ADC Pusher Frequency (µs)` | µs | Per-function pusher cycle override (if set) |
| `Calibration` | - | Calibration mode: `Dynamic 2` = lock-mass, `Static` = fixed |
| `Set Mass` | Da | Precursor m/z of an MS/MS function. Present on MS/MS sections (`TOF MSMS`, `TOF DAUGHTER`, and the DDA MS/MS slots labelled `TOF SURVEY` / `MOBILITY SURVEY`); absent from every `TOF PARENT FUNCTION` (MSe) section. DDA slots store `0.0`, which the parser reports as absent (`set_mass_da = None`), never as m/z 0. In DDA bundles a non-zero value is a method default, not each scan's precursor (PXD001123, PXD010569: one value per function while the scans' precursors differ); the per-scan precursor is the `Set Mass` channel of [`_FUNCnnn.STS`](07-func-sts.md) |
| `... Collision Energy Ramp Start` / `... Ramp End` | eV | Collision-energy ramp of the function, e.g. `Transfer Collision Energy Ramp Start (eV)  27.0` |
| `... Collision Energy Low` / `... High` | eV | Same, other spelling, e.g. `Transfer MS Collision Energy Low (eV)  25.0` |
| `... Collision Energy (eV)  <a> (Ramping to <b>)` | eV | Same, on one line, e.g. `Using Auto Transfer MS Collision Energy (eV)  15.0000 (Ramping to 25.0000)` |

The three ramp spellings are parsed into
`ExternFunction::collision_energy_ramp_ev = Some((start, end))` when
`end > start` (a `0.0` to `0.0` ramp is no ramp).

No charge state or isolation-width field has been found on any function
type in any corpus sample; `Set Mass` is the only precursor field
`_extern.inf` exposes. Per-scan collision energy for MS/MS-classified
functions (both targeted and MSe) comes from a separate file - see
[07 - _FUNCnnn.STS](07-func-sts.md)'s "Collision Energy" channel.

`Start Mass` / `End Mass` are not decoded. Spectrum decoding uses
`_FUNCTNS.INF`'s `FunctionInfo::mz_low` / `mz_high`. The corpus has not
established whether the two pairs are aliases or bounds with different
meanings, and `TOF MSMS FUNCTION` sections use an `MSMS End Mass` key
instead of `End Mass` (Sigilweaver/OpenWRaw#24).

## MS Level

`Reader::ms_level` assigns each function an MS level from the section
label, the section's collision-energy ramp and the function's position:

1. `REFERENCE`: MS1.
2. `TOF MSMS`, `TOF DAUGHTER`: MS2.
3. `TOF MS`: MS2 only when the section records a collision-energy ramp and
   another non-lock-mass `TOF MS` function of the same bundle records
   none (the low-energy half of an MSe pair). Otherwise MS1.
4. Any other label (`TOF PARENT`, `TOF SURVEY`, `MOBILITY SURVEY`, ...):
   function 1, or the only function, is MS1; later functions are MS2.

Rule 3 is deliberately narrow. A ramp on a `TOF MS` function with no
unramped partner, or a pair of `TOF MS` functions with no collision-energy
fields at all, stays MS1, because the file then gives no unambiguous sign
of which function fragments. In the public corpus rule 3 applies to 3
bundles: PXD009047 (function 2, `Transfer Collision Energy Ramp Start/End`
27 to 50 eV) and both PXD078353 bundles (function 2, `15.0000 (Ramping to
25.0000)`). The 45 Vion bundles exported by UNIFI (PXD081045) have two
`TOF MS` functions and no collision-energy field in `_extern.inf` or any
`_FUNCnnn.STS`, so both are reported as MS1 even though function 2 is
probably the high-energy function. PXD066594 has one `TOF MS` function
with a fixed 30 eV transfer collision energy and stays MS1.

The two functions of an MSe pair have byte-identical `_FUNCTNS.INF`
records (PXD009047, PXD078353, PXD081045), so that file does not help.

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
- Used by: `Reader::ms_level` (labels, collision-energy ramps) and
  `Reader::precursor_mz` (`Set Mass`, when the scan's STS file has no
  non-zero `Set Mass`). `ExternInf::a_us()` (flight-time constant from `Lteff`/`Veff`) is parsed but not used by any DAT decoder, since every encoding stores m/z directly
