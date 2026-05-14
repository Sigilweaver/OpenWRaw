# OpenWRaw

Open-source, cross-platform reader for the Waters MassLynx RAW mass
spectrometry data format.

Waters RAW is the native format produced by Waters LC-MS instruments
(Synapt, Xevo, ACQUITY, and related product lines). No open-source,
dependency-free reader currently exists: all existing tools either
require the proprietary Windows-only MassLynx SDK or wrap it via
ProteoWizard on Windows.

OpenWRaw reverse-engineers the format from a corpus of public datasets
and implements a clean reader in Rust with no external dependencies.

## Status: v0.1.0

Core binary format decoded and validated against three instrument classes:

| Instrument class | Encoding | IDX Variant | Status |
|-----------------|----------|-------------|--------|
| QTOF Ultima (older) | A (6-byte records) | A (22-byte) | Decoded |
| SYNAPT G2-Si (IMS) | B (8-byte IMS cells) | B (30-byte) | Decoded |
| Xevo G2-XS QTof | C (8-byte sub-bin) | B (30-byte) | Decoded |

Parsers implemented and corpus-validated:
- `_HEADER.TXT` - acquisition metadata and calibration polynomials
- `_extern.inf` - instrument geometry (Lteff, Veff, pusher interval)
- `_FUNCTNS.INF` - function descriptor table (type, subtype, mass range)
- `_FUNCnnn.IDX` - scan index (Variant A 22-byte and Variant B 30-byte)
- `_FUNCnnn.DAT` - spectrum data (Encodings A, B, C with m/z decoding)
- `_CHROMS.INF` + `_CHROnnnn.DAT` - chromatographic channel metadata and data

## Repository Structure

```
corpus/         Sample .raw directories from PRIDE (gitignored)
analysis/       Python/uv tooling for corpus acquisition and binary analysis
crates/
  openwraw/     Core Rust library (no external dependencies, 69 tests)
  openwraw-cli/ CLI tool
  openwraw-py/  PyO3 Python bindings
docs/
  format/       Reverse-engineered format documentation (11 spec files)
re/             Working notes (gitignored)
```

## Building

Requires: Rust 1.75+ (stable), cargo

```sh
cargo build --workspace
cargo test --workspace
```

## CLI Usage

```sh
# Inspect metadata of a .raw directory
openwraw inspect path/to/sample.raw

# Convert all MS functions to mzML
openwraw convert path/to/sample.raw -o output.mzML

# Convert a single function
openwraw convert path/to/sample.raw -o output.mzML --function 1
```

## Library Usage

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

## Python Usage

Requires: Python 3.8+, maturin

```sh
pip install maturin
maturin develop
```

```python
import openwraw

r = openwraw.RawReader("sample.raw")
print(r.functions)        # list of FunctionInfo

# Read a 1-D spectrum (Encoding A or C)
spec = r.read_spectrum(1, 0)
print(spec.mz[:5], spec.intensity[:5])

# Read a full IMS spectrum (Encoding B, SYNAPT)
ims = r.read_ims_spectrum(1, 0)
print(ims.mz[:3], ims.drift_time_ms[:3])

# Chromatographic channels
for ch in r.channels:
    pts = r.read_chrom(ch.index)
    print(ch.name, ch.units, len(pts), "points")
```

## Analysis Tooling

Requires: Python 3.12+, uv

```sh
cd analysis
uv sync
```

## License

MIT

