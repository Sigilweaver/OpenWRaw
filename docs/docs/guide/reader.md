---
sidebar_position: 1
---

# Reader

Loading a `.raw` directory parses the metadata files
(`_HEADER.TXT`, `_extern.inf`, `_FUNCTNS.INF`, `_CHROMS.INF`) and
indexes every MS function and chromatogram. Spectrum data is read on
demand from the corresponding `_FUNCnnn.DAT` files.

```rust
use openwraw::Reader;

let r = Reader::open("sample.raw")?;
println!("{} functions", r.functions.len());
```

Public types live under `openwraw::raw`:

| Module          | Purpose                                        |
| --------------- | ---------------------------------------------- |
| `header`        | `_HEADER.TXT` - metadata + calibration        |
| `extern_inf`    | `_extern.inf` - instrument geometry           |
| `functions_inf` | `_FUNCTNS.INF` - function descriptors         |
| `index`         | `_FUNCnnn.IDX` - scan index (Variant A / B)   |
| `data`          | `_FUNCnnn.DAT` - spectrum decoders            |
| `chroms`        | `_CHROMS.INF` + `_CHROnnnn.DAT`                |

The high-level `Reader` glues functions and their spectrum data together;
chromatogram channels are a separate concern accessed directly through the
`chroms` module (see [Chromatograms](./chromatograms)). For byte-level
access the parsers under `openwraw::raw` are also usable individually.

Note: the Python bindings expose this same reader as `openwraw.RawReader`
(see [Quickstart](../quickstart)) - the "Raw" prefix is a Python-side
naming choice, not the Rust type name.

## Error handling

Public functions return `openwraw::Result<T>`. The error type
(`openwraw::Error`) reports the failing file and the underlying cause.
