---
sidebar_position: 3
---

# Quickstart

## CLI

```sh
# Inspect a .raw directory
openwraw inspect path/to/sample.raw

# Convert all MS functions to mzML
openwraw convert path/to/sample.raw -o output.mzML

# Convert a single function
openwraw convert path/to/sample.raw -o output.mzML --function 1
```

## Rust

```rust
use openwraw::Reader;

let reader = Reader::open("sample.raw")?;
for function in &reader.functions {
    println!(
        "function {}: {} scans, encoding {:?}",
        function.index,
        function.scan_count(),
        function.encoding
    );
}

// Every non-lock-mass scan, in function then scan order.
for scan in reader.iter_spectra() {
    let scan = scan?;
    println!(
        "function {} scan {}: RT={:.2} min, {} points",
        scan.function_index,
        scan.scan_idx,
        scan.retention_time_min,
        scan.spectrum.mz.len()
    );
}
```

m/z is calibrated with the polynomial in `_HEADER.TXT`; no lock-mass
correction is applied. Ion mobility is not decoded.

## Python

```python
import openwraw

r = openwraw.RawReader("sample.raw")
print(r.functions)         # list of FunctionInfo

# Calibrated m/z and intensity for function 1, scan 0
spec = r.read_spectrum(1, 0)
print(spec.mz[:5], spec.intensity[:5])

# Chromatographic channels
for ch in r.channels:
    pts = r.read_chrom(ch.index)
    print(ch.name, ch.units, len(pts), "points")
```

## Next

- [Reader API](./guide/reader)
- [Encodings](./guide/encodings)
- [Ion mobility](./guide/ims)
- [Format specification](./format/overview)
