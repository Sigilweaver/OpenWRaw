---
sidebar_position: 3
---

# Ion mobility (IMS)

SYNAPT-class instruments produce ion-mobility data via Encoding B, in
which each scan is subdivided into mobility cells (sub-bins) keyed by
drift time. `read_ims_spectrum` returns the full flat spectrum with a
per-peak `drift_time_ms` column.

```rust
use openwraw::RawReader;

let r = RawReader::open("synapt.raw")?;
let ims = r.read_ims_spectrum(1, 0)?;
for (mz, dt, intensity) in itertools::izip!(&ims.mz, &ims.drift_time_ms, &ims.intensity) {
    println!("{mz:.4}\t{dt:.3}\t{intensity:.0}");
}
```

In Python the same data is exposed via NumPy arrays:

```python
ims = r.read_ims_spectrum(1, 0)
print(ims.mz.dtype, ims.drift_time_ms.shape, ims.intensity.shape)
```

The drift time is derived from the sub-bin index and the pusher
interval reported by `_extern.inf` (the `a_us` field).
