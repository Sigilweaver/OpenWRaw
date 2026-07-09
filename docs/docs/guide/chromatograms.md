---
sidebar_position: 4
---

# Chromatograms

Chromatographic channels (TIC, BPI, UV, analog signals) are described
by `_CHROMS.INF` and stored in `_CHROnnnn.DAT`. Each channel has an
index, a name, and engineering units.

```rust
use openwraw::raw::chroms::{read_chro_dat, ChromsInf};
use std::path::Path;

let dir = Path::new("sample.raw");
let ci = ChromsInf::from_path(&dir.join("_CHROMS.INF"))?;
for ch in &ci.channels {
    let chro_num = ci.chro_number_for_channel(ch.index);
    let pts = read_chro_dat(&dir.join(format!("_CHRO{chro_num:04}.DAT")))?;
    println!("{}\t{}\t{} points", ch.name, ch.units, pts.len());
}
```

Chromatogram channels are accessed directly through the `chroms` module
rather than through `Reader` - `Reader::open` only indexes MS functions.

Each point is `(retention_time_min, value)`. The encoding is fixed and
does not require calibration.

See the [format specification](../format/chroms-inf) for the
`_CHROMS.INF` byte layout and [chro-dat](../format/chro-dat) for the
data file.
