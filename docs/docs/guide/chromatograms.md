---
sidebar_position: 4
---

# Chromatograms

Chromatographic channels (TIC, BPI, UV, analog signals) are described
by `_CHROMS.INF` and stored in `_CHROnnnn.DAT`. Each channel has an
index, a name, and engineering units.

```rust
use openwraw::RawReader;

let r = RawReader::open("sample.raw")?;
for ch in r.channels() {
    let pts = r.read_chrom(ch.index)?;
    println!("{}\t{}\t{} points", ch.name, ch.units, pts.len());
}
```

Each point is `(retention_time_min, value)`. The encoding is fixed and
does not require calibration.

See the [format specification](../format/chroms-inf) for the
`_CHROMS.INF` byte layout and [chro-dat](../format/chro-dat) for the
data file.
