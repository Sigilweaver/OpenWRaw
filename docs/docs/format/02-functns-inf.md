# _FUNCTNS.INF

Binary function descriptor table. Present in every .raw directory.
One record per acquisition function (survey MS, IMS, MS/MS, lock-mass reference, etc.).

## Status: Mostly Decoded (Phase 3)

Cross-referenced against `_extern.inf` "Scan Time" and "Interscan Time" fields and
IDX retention-time sequences for all four corpus instruments.

Corpus used:
- **PXD058812** - Synapt HDMS (non-IMS QTOF mode), 1 function
- **PXD066594/WANG.raw** - SYNAPT G2-Si IMS, 1 function
- **PXD068881/CtpA** - SYNAPT G2-Si DDA, 3 functions
- **PXD075602/DHPR** - Xevo G2-XS, 3 functions

## Record Layout (416 bytes per function, no file header)

All unlisted byte positions are zero in the entire corpus.

| Offset | Size | Type | Confirmed | Description |
|--------|------|------|-----------|-------------|
| 0x000  | 1    | u8   | Yes       | function_type_code: 0x12 (=18) in all corpus records; consistent with Waters SDK "TOF MS" category |
| 0x001  | 1    | u8   | Partial   | scan_subtype: 0x25 = older non-IMS QTOF; 0x71 = G2-Si/G2-XS survey; 0xf1 = G2-Si/G2-XS lock-mass reference (= 0x71 | 0x80) |
| 0x002  | 4    | f32  | Yes       | cycle_contribution (s) = scan_time + interscan_delay; confirmed as scan_time + interscan_delay for all 9 records |
| 0x006  | 4    | f32  | Yes       | interscan_delay (s), duplicate of 0x01C |
| 0x00A  | 6    | -    | -         | zero |
| 0x010  | 2    | u16  | Yes       | tof_depth: number of TDC bins per pusher pulse (e.g. 16672 for G2-Si at 69 µs, 17204 for older QTOF at 62 µs) |
| 0x012  | 2    | -    | -         | zero |
| 0x014  | 3    | -    | -         | zero |
| 0x017  | 1    | u8   | Partial   | 0x01 in all corpus records; possibly continuum/centroid flag (1=continuum) or polarity (1=positive) |
| 0x018  | 4    | -    | -         | zero |
| 0x01C  | 4    | f32  | Yes       | interscan_delay (s) - idle time between end of one scan and start of next; matches `Interscan Time (sec)` in `_extern.inf` |
| 0x020  | 4    | f32  | Yes       | scan_time (s) - data collection time per scan; matches `Scan Time (sec)` in `_extern.inf` |
| 0x024  | 124  | -    | -         | zero |
| 0x0A0  | 4    | f32  | Yes       | mz_low - mass range lower bound (m/z) |
| 0x0A4  | 124  | -    | -         | zero |
| 0x120  | 4    | f32  | Yes       | mz_high - mass range upper bound (m/z) |
| 0x124  | 156  | -    | -         | zero |

Total record size: 416 bytes.

## Timing Model

For an experiment with M multiplexed functions alternating in a DDA cycle:

```
per-function slot (s) = scan_time + interscan_delay   [field at 0x002]
total DDA cycle (s)   = sum of slots across all M active functions
scan interval for Fn_i = total DDA cycle
```

Cross-check validation (all from IDX RT diffs vs FUNCTNS.INF):

| Dataset      | Fns | scan_time | interscan | slot (s) | observed interval (s) |
|--------------|-----|-----------|-----------|----------|-----------------------|
| P15 QTOF     | 1   | 1.00      | 0.10      | 1.10     | 1.11 (single fn)      |
| WANG G2-Si   | 1   | 1.00      | 0.014     | 1.014    | 1.02 (single fn)      |
| CtpA Fn1-2   | 2   | 0.30      | 0.014     | 0.314    | 0.628 (2x slot)       |
| DHPR Fn1-2   | 2   | 0.50      | 0.014     | 0.514    | 1.03 (2x slot)        |

## TOF Depth Field

`tof_depth` at +0x010 is the number of TDC bins per pusher pulse.
It equals `floor(pusher_interval_µs * 1000 / tdc_bin_ns)`.

| Dataset   | pusher (µs) | tof_depth | implied TDC bin (ns) |
|-----------|-------------|-----------|----------------------|
| P15 QTOF  | 62          | 17204      | 3.60                |
| WANG G2-Si| 69          | 16672      | 4.14                |
| CtpA G2-Si| 69          | 16704      | 4.13                |
| DHPR G2-XS| 60.25       | 16800      | 3.59                |

## Scan Subtype (byte 0x001)

Observed values and correlations:

| Value | Instrument class | Function role |
|-------|-----------------|---------------|
| 0x25  | Older non-IMS QTOF (Synapt HDMS base) | Single survey function |
| 0x71  | SYNAPT G2-Si / Xevo G2-XS | Survey / parent function |
| 0xf1  | SYNAPT G2-Si / Xevo G2-XS | Lock-mass / reference function |

Bit 7 (0x80) of the subtype byte appears to flag a lock-mass reference function.
Polarity cannot be confirmed from the available corpus (all positive-mode data).

## Key Facts

- File size = N x 416 bytes, where N = number of acquisition functions
- No file header; first byte of file is first byte of function 1 record
- Functions 1..N correspond to `_FUNC001.DAT/.IDX/.STS`, etc.
- The `_extern.inf` "Function Parameters - Function N" sections provide matching
  human-readable copies of scan_time, interscan_delay, and mass range

## Undecoded

- Exact semantics of function_type_code 0x12 (consistent with Waters SDK enum
  value 18 = "TofMS" but cannot be confirmed without non-TOF corpus data)
- Byte 0x017 purpose (polarity vs data format flag)
- Bytes 0x012-0x013, 0x014-0x016, 0x018-0x01B (all zero in corpus)
- IMS-specific method parameters (wave velocity, trap bias, transfer bias) are not
  present in this record. They do not appear in `_extern.inf` either. These values
  are likely stored in a proprietary instrument method file outside the `.raw`
  directory and are not recoverable from the raw data alone.

## Reference Sources

- `_extern.inf` "Scan Time (sec)" and "Interscan Time (sec)" per function
- Empirical hex analysis: `re/src/analysis/inspect.py`
