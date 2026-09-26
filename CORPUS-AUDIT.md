# Waters corpus decode audit - 2026-09-26

The expanded corpus at `/mnt/nas/Data/WRaw/` contains 80 RAW bundles from
25 verified PRIDE archives. The original 2026-09-25 audit found 16 bundles
that decoded every non-lock-mass scan, 5 with scan errors, and 59 that failed
to open. The previous success rate was 16/80 bundles (20%).

After the parser and decoder changes on `fix/waters-corpus-decode`, every
bundle opens and every non-lock-mass scan decodes:

| Measure | Result |
| --- | ---: |
| Bundles fully decoded | 80/80 (100%) |
| Scans decoded | 250,236/250,236 (100%) |
| Scans with at least one peak | 215,815 |
| Scan errors | 0 |
| Vion/UNIFI bundles fully decoded | 45/45 |
| Vion/UNIFI scans decoded | 91,792/91,792 |

The audit identified four format causes: missing or differently spelled
pusher timing, lowercase or prefixed side-file names, 8-byte DAT records
behind 22-byte IDX files, and additional 6-byte marker layouts. Some IDX
file lengths also fit both the 22-byte and 30-byte record strides. The
Python binding previously used its own decoder and file lookup; it now uses
the core reader for spectra.

To repeat the audit:

```sh
cargo run -p openwraw --release --example audit_corpus -- /mnt/nas/Data/WRaw
```

The audit checks whether scans parse and decode without errors. It does not
establish mass accuracy against reference spectra. Empty decoded scans are
counted as decoded and reported separately through the nonempty count.
