# Migrating to OpenWRaw 5.0

OpenWRaw 5.0 builds on openmassspec-core 4.0. Core's own changes (types, the
mzML writer's checks, Arrow) are in
[MIGRATING-4.0.md](https://github.com/Sigilweaver/OpenMassSpecCore/blob/main/MIGRATING-4.0.md).
This page covers what OpenWRaw changes on top.

## Python

Python 3.10 or newer. Older interpreters stay on OpenWRaw 4.x.

Record dicts (`RawReader.read_record`, `iter_records`) change in these keys:

| 4.x | 5.0 |
|-----|-----|
| `rec["precursor"]["collision_energy"]` (float or `None`) | `rec["precursor"].get("collision_energies", [])`, a list of `{"value": float, "unit": "ev" \| "nce" \| None}` |
| `rec["precursor"]["ce_is_nce"]` | the `unit` of each entry |
| `rec["analyzer"] == "tofms"` | `rec["analyzer"] == "tof"` |
| `rec["scan_mode"]` always `"profile"` or `"centroid"` | also `None` when unknown (LCT Premier) |

`collision_energies` is absent from the dict when empty. An empty list is
common: a function with a collision-energy ramp, or with two collision-energy
channels, states no collision energy. `rec["extra"]["openwraw.collision_energy_unstated"]`
says why, and `RawReader.scan_parameters(function, scan)["collision_energy_ev"]`
still returns the raw channel.

```python
# 4.x
ce = rec["precursor"]["collision_energy"]

# 5.0
energies = rec["precursor"].get("collision_energies", [])
ce = energies[0]["value"] if energies else None
```

## Rust

- `openmassspec-core = "4"`.
- `Encoding::is_centroided()` is now `FunctionEntry::is_centroided()` and
  returns `Option<bool>`.
- `ExternFunction` has a new field, `collision_energy_labelled_ev`. The struct
  is `#[non_exhaustive]`, so only code that built one by hand is affected.
- Records, `WatersSource` and `WriteSummary` are Core 4 types: collision
  energy is `PrecursorInfo::collision_energies`, retention time is
  `Option<f64>` (always `Some` from this reader).

## mzML conversion

Core 4's writer refuses records it used to write. For Waters bundles that is:

| Case | Check to allow |
|------|----------------|
| MS/MS record with a collision energy whose `_extern.inf` section has no `(eV)` label (older Q-Tof bundles) | `unknown_collision_energy_unit` |
| LCT Premier spectra (no scan mode) | `unknown_scan_mode` |

A chromatogram channel with a NaN sample is refused with no check to allow; use
`RecordErrorPolicy::Skip` to leave it out.
