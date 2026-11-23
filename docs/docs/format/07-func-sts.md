# _FUNCnnn.STS

Binary scan statistics file. One file per function (e.g. `_FUNC001.STS`,
`_FUNC002.STS`). Records per-scan instrument housekeeping values: electrode
voltages, collision energies, push counts, lock-mass corrections, and TIC
traces.

## Status: Fully Decoded

## File Layout

```
[32-byte file preamble]
[n_desc × 48-byte descriptor records]      (starting at offset 0x20)
[n_scans × scan_record_size bytes of data]
```

Total header size = 32 + n_desc × 48. This equals the `data_offset` stored
at preamble byte 0.

## File Preamble (32 bytes)

| Offset | Type | Description |
|--------|------|-------------|
| 0x00   | u16  | `data_offset` - byte offset where per-scan data begins |
| 0x02   | u16  | Format version (always 1 observed) |
| 0x04   | u16  | `scan_record_size` - bytes per scan in the data section |
| 0x06   | u16  | `n_desc` - number of descriptor records |
| 0x08-0x1F | - | Zeroes (padding to 32 bytes) |

Equations:
- `data_offset = 32 + n_desc * 48`
- `n_scans = (file_size - data_offset) / scan_record_size`

Validated:
- PXD058812 P15: data_offset=0x4D0=1232, scan_sz=63, n_desc=25, n_scans=197
- PXD075602 DHPR: data_offset=0x0AA0=2720, scan_sz=167, n_desc=56, n_scans=1150

## Descriptor Records (48 bytes each)

Each descriptor defines one channel stored in the per-scan data section.

| Offset within record | Type | Description |
|---------------------|------|-------------|
| 0x00 | u16 | Channel sequence number (not always contiguous; some skip) |
| 0x02 | u16 | Encoding type (see table below) |
| 0x04 | u16 | Byte offset of this channel within each scan record |
| 0x06-0x2F | bytes | Null-padded ASCII channel name (42 bytes) |

### Encoding Types

| Type code | C type | Size | Notes |
|-----------|--------|------|-------|
| 0 | u8 | 1 byte | Boolean flag / small integer |
| 1 | i16 | 2 bytes | Signed 16-bit integer |
| 2 | u32 | 4 bytes | Unsigned 32-bit integer |
| 3 | f32 | 4 bytes | IEEE 754 single-precision float |

The byte offset in the descriptor (`0x04`) is used directly as an index
into the per-scan record. Channels are packed without explicit gaps; the
offset of channel k+1 equals offset k + size(type k).

## Per-Scan Data

The data section starts at `data_offset` and contains `n_scans` consecutive
records, each `scan_record_size` bytes. To read channel X for scan S:

```
channel_value = scan_data[S * scan_record_size + descriptor[X].offset]
```

decoded using the descriptor's encoding type.

## Known Channels

Observed across all four corpus datasets. Channels available depend on
instrument generation; newer firmware adds channels.

| Seq | Name | Type | Code | Notes |
|-----|------|------|------|-------|
| 13 | Segment Number | i16 | varies | 0 for normal scans |
| 52 | Cone | i16 | varies | Cone voltage (V); matches `_extern.inf` Sampling Cone |
| 62 | Collision Energy | f32 | varies | Per-scan collision energy. The file records no unit. Matches the `_extern.inf` field labelled Trap where it labels a trap and a transfer energy; on ramped functions it holds the start of the ramp (see below) |
| 76 | Pusher Frequency | varies | varies | Function-dependent |
| 78 | Collision Energy2 | f32 | varies | Second collision energy. Matches the `_extern.inf` field labelled Transfer where it labels both. Absent from older Q-Tof bundles |
| 77 | Set Mass | f32 | 3 | Precursor m/z of an MS/MS scan; 0 on MS1, MSe and lock-mass scans |
| 100 | Reference Scan | u8 | varies | 1 if this scan is a lock-mass reference |
| 101 | Use lock mass correction | u8 | varies | Boolean |
| 102 | Lock mass correction | f32 | varies | Mass correction applied (Da) |
| 103 | Use temp correction | u8 | varies | Boolean |
| 104 | Temperature correction | f32 | varies | Temperature-derived TOF correction (µs) |
| 107 | TIC Trace A | f32 | varies | Total Ion Current (a.u.) for function A |
| 108 | TIC Trace B | f32 | varies | Total Ion Current (a.u.) for function B |
| 116 | Scan Push Count | u32 | varies | Number of ADC pushes summed in this scan |
| 117 | Scanwave Normalisation Factor | f32 | varies | IMS normalisation factor |
| 121 | ETD Fragmentation Mode | i16 | varies | 0 = CID; non-zero = ETD |

Not all channels are present in every file. Parse by looking up the
descriptor list rather than assuming fixed offsets.

## Set Mass (seq 77)

In DDA bundles the `_extern.inf` `Set Mass` of each MS/MS function is 0 or
a fixed method default, while this channel holds each scan's precursor
m/z. Every MS/MS scan of every DDA function in the corpus has a non-zero
value; every MS1, MSe and lock-mass scan has 0.

Cross-check against `_auto.txt`, a plain-text `m/z,time (s)` log of the
precursors chosen during acquisition, present in 6 corpus bundles. Each
run of consecutive MS/MS scans with one `Set Mass` value starting within
30 s after a log entry was matched to it:

| Bundle | `_auto.txt` entries | MS/MS runs | Matched |
|--------|--------------------:|-----------:|--------:|
| PXD041695/JLG-1D-ISY-C18_Q0764.raw | 410 | 410 | 410 within 0.5 mDa |
| PXD041695/JLG-1D-ISY-C8_Q0753.raw | 412 | 412 | 412 within 0.5 mDa |
| PXD010569/20161025_AV_Dario_Banda_10.raw | 274 | 274 | 274 within 0.5 mDa |
| PXD010569/20161025_AV_Dario_Banda_3.raw | 273 | 273 | 273 within 0.5 mDa |
| PXD001123/SiRNA_21-2.raw | 882 | 865 | 865 within 0.5 mDa |
| PXD029515/blast_young_0h_H1__MSMS.raw | 1217 | 1217 | 1217 within 100 ppm (median +18 ppm) |

In PXD001123, 17 logged precursors have no MS/MS scan. In PXD029515 the
log and the channel differ by a constant offset of about 18 ppm; the
channel value is the one reported. PXD003126 and PXD073126 (mobility DDA)
have no `_auto.txt`, but their MS/MS scans carry the channel the same way.
For the targeted MS/MS sample PXD035818 the channel reads 884.92 on every
scan, where `_extern.inf` gives `Set Mass 884.9`.

## Observed Values

| Dataset | n_desc | scan_sz | Cone | Collision Energy | Push Count |
|---------|--------|---------|------|-----------------|------------|
| PXD058812 P15 (non-IMS QTOF) | 25 | 63 | 30 V | 10.0 eV | - |
| PXD075602 DHPR (Xevo G2-XS) | 56 | 167 | 30 V | 4.0 eV | 8299 |

Push Count = 8299 in DHPR: 8299 × 60.25 µs ≈ 0.50 s per scan, consistent
with the 0.50 s scan time recorded in `_FUNCTNS.INF`.

## Relationship to IDX Fields

The IDX `+0x08` field (30-byte Variant B, value labelled "Large value / TIC?")
likely corresponds to one of the TIC Trace channels here. This cross-check
has not yet been confirmed empirically.

## Rust Implementation

`crate::raw::func_sts::FuncSts` parses this file and exposes per-scan
channel lookups by name; `FuncSts::collision_energy(scan_idx)` reads the
"Collision Energy" channel specifically, and
`FuncSts::etd_fragmentation_mode(scan_idx)` reads the "ETD Fragmentation
Mode" channel (seq 121) specifically, and
`FuncSts::set_mass(scan_idx)` reads the "Set Mass" channel (seq 77), with a
stored 0 returned as `None`. `Reader::precursor_mz` uses it as the
precursor m/z of each scan, falling back to the `_extern.inf` `Set Mass`
(also `None` when 0), and the mzML records write that value as both the
isolation window target m/z and the selected ion m/z.
`Reader::open` parses every function's
`_FUNCnnn.STS` when present (silently `None` if absent or malformed - this
file is supplementary, not required to decode peak data), and
`mzml::record_from_scan` decides what collision energy the bundle states for
a function's scans (`mzml::collision_energy_statement`):

- A function whose `_extern.inf` section records a collision-energy ramp has no
  single energy per scan. The channel holds the start of the ramp: every scan
  of PXD075602 `DHPR_11257-1.raw` function 2 reads 15.0 for a 15 to 40 eV ramp.
  No collision energy is stated.
- A function whose STS file also has "Collision Energy2" (seq 78) has two
  energies per scan. On the trap and transfer instruments of the public corpus
  seq 62 matches the `_extern.inf` trap field and seq 78 the transfer field (PXD071342 function 2:
  4.0 and 27.0 for a 27 to 47 eV transfer ramp; PXD002393 function 2: 16.0
  and 2.0 for a 16 to 40 eV trap ramp; PXD073126 DDA: 4.0 and 22 to 44 per
  scan). Nothing in the bundle says which cell fragments the precursor, so no
  collision energy is stated.
- Otherwise the "Collision Energy" channel is the energy of each scan
  (PXD041695, PXD010569). Its unit is eV when the function's `_extern.inf`
  section labels a collision energy `(eV)`, and unknown otherwise.

When no collision energy is stated for an MS/MS record, the reason is in its
`extra` map under `openwraw.collision_energy_unstated`, and both raw channels
stay under `openwraw.sts.62.Collision Energy` and
`openwraw.sts.78.Collision Energy2`. The record's `<precursor>` still carries
the precursor m/z and the activation.

`PrecursorInfo::activation` comes from the ETD Fragmentation Mode channel:
`0` maps to `Activation::CID`,
non-zero to `Activation::ETD` (Sigilweaver/OpenWRaw#22). Every ETD
Fragmentation Mode value seen in the repository corpus is `0`; the non-zero
branch follows directly from this table's documented encoding but has not
been exercised against a real non-zero fixture, only synthetically
(`mzml::tests::activation_for_maps_zero_to_cid_and_nonzero_to_etd`).

Channels 101 and 102 remain available through the generic `channel` /
`value_at` API but are deliberately not applied downstream. Every sample in
the repository corpus has `Use lock mass correction = 0` and
`Lock mass correction = 0.0`. Consequently the corpus cannot establish how a
non-zero lock-mass value modifies the calibrated TOF-to-m/z conversion.
Applying an interpretation would require a fixture with a non-zero value and
an independently checkable in-repository invariant (Sigilweaver/OpenWRaw#23).

## Reference Sources

- Empirical analysis: all `_FUNC001.STS` files in corpus
- Corpus samples:
  - PXD058812/molecular_mass_P15_01.raw (25 channels, 63 bytes/scan)
  - PXD075602/DHPR_11257-1.raw (56 channels, 167 bytes/scan)
  - PXD035818/17122018_TNFA_PEPTIDE_GSHH_MSMS_884.raw (targeted MS/MS,
    confirms the same descriptor/channel layout on a non-MSe function)
