# Waters corpus decode audit - 2026-09-26

The expanded corpus at `/mnt/nas/Data/WRaw/` contains 80 RAW bundles from
25 verified PRIDE archives. The audit reports two things that should not be
confused:

1. **Parse coverage**: does every scan decode without an error?
2. **m/z accuracy**: do lock-mass (reference) functions show their reference
   compound where it belongs?

A scan that parses is not necessarily a correct spectrum. An earlier version
of this report counted parse coverage alone as a 100% success rate while the
Vion/UNIFI lock mass was off by up to 3,400 ppm.

## Parse coverage

| Measure | Result |
| --- | ---: |
| Bundles opened and fully decoded | 80/80 |
| Non-lock-mass scans decoded | 250,236/250,236 |
| Scans with at least one peak | 215,939 |
| Scan errors | 0 |

## m/z accuracy on lock-mass functions

Each lock-mass function is checked against `ReferenceMass1` from
`_extern.inf` when present, otherwise against leucine enkephalin [M+H]+
(556.2766) or [Glu1]-fibrinopeptide B [M+2H]2+ (785.8421). A peak counts
only if its first 13C isotope appears at the expected spacing. The tolerance
is 100 ppm on the median over up to 20 scans; this is raw data before
lock-mass correction.

| Encoding | Lock functions | Pass | Fail | No reference peak |
| --- | ---: | ---: | ---: | ---: |
| A (6-byte) and D (8-byte, 22-byte index) | 54 | 51 | 1 | 2 |
| B and C (8-byte, 30-byte index) | 18 | 0 | 4 | 14 |

Encodings A and D store m/z as a floating-point word; see
`docs/docs/format/04-func-dat.md` for the layout and its validation.

- Vion/UNIFI (45 runs, issue #33): +20 to +69 ppm, with one offset per
  acquisition batch. The saturated monoisotopic Leu-Enk peak centroids
  10-20 ppm above its isotopes.
- SYNAPT G2, BsNb, and older QTof files: -11 to +38 ppm.
- PXD029515 `blast_young_0h_H1__MSMS.raw` fails at -153 ppm. Its Glu-fib
  envelope is present with correct isotope spacing and a consistent offset;
  PXD003126, with identical `_extern.inf` geometry and software, reads
  +24 ppm. This is attributed to the instrument's calibration at acquisition
  time, which is not proven.
- PXD010569 lock functions contain a cluster series 97.98 Da apart rather
  than Leu-Enk or Glu-fib, so they are not scored.

**Encodings B and C are not yet accurate.** Their decoders anchor the first
and last record of each scan to the declared mass range. None of their lock
functions passes; four fail by 134-645 ppm and the rest do not show an
isotope-confirmed reference at all. Every 30-byte-index function sampled
passes the same floating-point m/z and ADC-sample checks as Encoding D,
which points to the fix.

## Reproduce

```sh
cargo run -p openwraw --release --example audit_corpus -- /mnt/nas/Data/WRaw
cargo run -p openwraw --release --example audit_corpus -- /mnt/nas/Data/WRaw --lock-only
```

The command exits non-zero if any bundle fails to decode or any lock-mass
function fails, so it currently exits 1 because of the B and C failures and
PXD029515.
