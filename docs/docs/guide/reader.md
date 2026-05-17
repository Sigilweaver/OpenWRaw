---
sidebar_position: 1
---

# Reader

Loading a `.raw` directory parses the metadata files
(`_HEADER.TXT`, `_extern.inf`, `_FUNCTNS.INF`, `_CHROMS.INF`) and
indexes every MS function and chromatogram. Spectrum data is read on
demand from the corresponding `_FUNCnnn.DAT` files.

```rust
use openwraw::RawReader;

let r = RawReader::open("sample.raw")?;
println!("{} functions, {} channels", r.functions().len(), r.channels().len());
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

The high-level `RawReader` glues these together; for byte-level access
the parsers are also usable individually.

## Error handling

Public functions return `openwraw::Result<T>`. The error type
(`openwraw::Error`) reports the failing file and the underlying cause.
