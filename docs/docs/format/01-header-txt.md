# HEADER.TXT

Plain ASCII key-value file. Always present. Encodes run-level metadata and
per-function TOF calibration polynomials.

## Status: Fully Known

HEADER.TXT is human-readable and straightforward to parse. Its presence
and basic structure are consistent across all known instrument generations.

## Example Content

```
$$ Sample Description:
$$ Acquired Date: 10-Sep-2019
$$ Acquired Time: 14:23:07
$$ Instrument: SYNAPT G2-Si#WAA123456789
$$ Operator: operator
$$ MS Method: D:\Projects\...\method.EXP
$$ Cal Function 1: -3.072e-2,9.999e-1,8.393e-5,-2.360e-6,2.529e-8,T1
$$ Cal Function 2: -3.072e-2,9.999e-1,8.393e-5,-2.360e-6,2.529e-8,T1
```

## Observations

- Lines beginning with `$$` are metadata fields
- Field name and value are separated by `: `
- Fields are not always present; treat all as optional
- Character encoding is 7-bit ASCII; no BOM observed
- Line endings are CRLF on Windows-generated files

## Calibration Polynomial Fields

Each MS function has a corresponding `$$ Cal Function N:` line where `N` matches
the 1-based function number. The value is a comma-separated list of floating-point
polynomial coefficients followed by a calibration-type code.

### Format

```
$$ Cal Function N: c0,c1,c2,...,ck,TYPE
```

- `N` = 1-based function index
- `c0..ck` = polynomial coefficients (5 or 6 coefficients observed)
- `TYPE` = calibration type: `T0` (none/identity), `T1` (standard TOF polynomial)
  - `T0` means no calibration (identity); all coefficients will be zero or absent
  - `T1` means the polynomial is applied to the raw TOF time before m/z conversion

### T1 Polynomial Definition

Given the raw TOF flight time `t_raw` (in microseconds), the calibrated flight
time is computed as a polynomial in `t_raw`:

```
t_cal = c0 + c1·t_raw + c2·t_raw² + c3·t_raw³ + ... + ck·t_raw^k
```

The calibrated time is then converted to m/z using the TOF physics formula:

```
mz = (t_cal / A_us)²
```

where `A_us` is the TOF constant derived from instrument geometry:

```
A_us = sqrt(m_proton · L_eff / (2 · e · V_eff)) × 10⁶
```

- `m_proton` = 1.6726219 × 10⁻²⁷ kg
- `L_eff` = effective flight path length in metres (from `_extern.inf` → `Lteff` field, convert mm → m)
- `e` = 1.6021766 × 10⁻¹⁹ C
- `V_eff` = effective accelerating voltage in volts (from `_extern.inf` → `Veff` field)

And the raw time `t_raw` in µs is derived from the stored TOF bin index:

```
t_raw = tof_bin × (pusher_cycle_us / 65536)
```

where `pusher_cycle_us` comes from `_extern.inf` (field `Pusher Cycle Time` or
`ADC Pusher Frequency`, both in µs).

### Observed Coefficients

| Dataset | Instrument | c0 | c1 | c2 | c3 | c4 | c5 | Type |
|---------|------------|-----|-----|-----|-----|-----|-----|------|
| PXD058812 | QTOF (native MS) | −3.072e-2 | 9.999e-1 | 8.393e-5 | −2.360e-6 | 2.529e-8 | *(absent)* | T1 |
| PXD075602 | Xevo G2-XS QTof | −4.778e-3 | 1.000e0 | 4.431e-6 | −1.461e-7 | −2.191e-10 | 5.119e-11 | T1 |

Note: the number of coefficients can vary between 5 and 6 (at least); parsers
must be tolerant of variable-length coefficient lists before the type code.

### Per-Function Calibration

When multiple MS functions are present, each function has its own `Cal Function N:`
line. In PXD075602, all three functions share the same coefficients (applied once at
acquisition time from the same lock-mass reference run).

### CoVar Fields

Additional lines `$$ Cal CoVar N:` contain the calibration covariance matrix as a
flat comma-separated list of 21 floats (row-major upper triangle of a 6×6 matrix).
These are used for mass accuracy estimation and are not required for basic decoding.

### StdDev Fields

`$$ Cal StdDev Function N:` gives the calibration residual standard deviation (Da).
A value of 0.0 indicates a perfect fit or a static calibration with no fit data.

## Reference Sources

- PXD058812/molecular_mass_P15_01.raw `_HEADER.TXT` (5-coefficient T1)
- PXD075602/DHPR_11257-1.raw `_HEADER.TXT` (6-coefficient T1, 3 functions)
- `_extern.inf` (Lteff, Veff, pusher cycle time)
