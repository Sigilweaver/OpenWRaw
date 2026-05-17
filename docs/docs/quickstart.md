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
use openwraw::raw::{
    header::Header,
    extern_inf::ExternInf,
    functions_inf::FunctionTable,
    index::ScanIndex,
    data::{decode_encoding_a, DecodeParams},
};
use std::path::Path;

let raw = Path::new("sample.raw");
let header = Header::from_path(&raw.join("_HEADER.TXT"))?;
let ext = ExternInf::from_path(&raw.join("_extern.inf"))?;
let funcs = FunctionTable::from_path(&raw.join("_FUNCTNS.INF"))?;

let f = &funcs.functions[0];
let params = DecodeParams {
    a_us: ext.a_us(),
    cal: header.cal_functions[&f.index].clone(),
    mz_low: f.mz_low as f64,
    mz_high: f.mz_high as f64,
    scan_time_ms: f.scan_time_s as f64 * 1000.0,
};

let idx_bytes = std::fs::read(raw.join(format!("_FUNC{:03}.IDX", f.index)))?;
let dat_bytes = std::fs::read(raw.join(format!("_FUNC{:03}.DAT", f.index)))?;
let ScanIndex::A(scans) = ScanIndex::from_bytes(&idx_bytes)? else { todo!() };

for scan in &scans {
    let start = scan.dat_offset as usize;
    let end = start + scan.n_records as usize * 6;
    let spectrum = decode_encoding_a(&dat_bytes[start..end], &params)?;
    println!("RT={:.2}min  {} peaks", scan.retention_time_min, spectrum.mz.len());
}
```

## Python

```python
import openwraw

r = openwraw.RawReader("sample.raw")
print(r.functions)         # list of FunctionInfo

# 1-D spectrum (Encoding A or C)
spec = r.read_spectrum(1, 0)
print(spec.mz[:5], spec.intensity[:5])

# Full IMS spectrum (Encoding B, SYNAPT)
ims = r.read_ims_spectrum(1, 0)
print(ims.mz[:3], ims.drift_time_ms[:3])

# Chromatographic channels
for ch in r.channels:
    pts = r.read_chrom(ch.index)
    print(ch.name, ch.units, len(pts), "points")
```

## Next

- [Reader API](./guide/reader)
- [Encodings A / B / C](./guide/encodings)
- [Ion mobility](./guide/ims)
- [Format specification](./format/overview)
