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
| Scans with at least one peak | 217,405 |
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
| A (6-byte) and D (8-byte), 22-byte index | 54 | 51 | 1 | 2 |
| D (8-byte) and E (12-byte), 30-byte index | 18 | 15 | 2 | 1 |

Encodings A, D and E store m/z as a floating-point word; see
`docs/docs/format/04-func-dat.md` for the layouts and their validation.

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

30-byte index (SYNAPT G2-S/G2-Si/XS, Xevo G2-XS, Xevo G3):

- 14 lock functions sit within 76 ppm (-75.6 to +66.5), including the
  12-byte Xevo G2-XS lock functions at +9.5 to +11.0 ppm.
- PXD069628 (Xevo G3): HC18_CE passes at -97.6 ppm and HC20_CE fails at
  -107.9 ppm.
- PXD001175 `S121126_06.raw` (SYNAPT G2-S) fails at +161.7 ppm on Glu-fib.
- In these three, the offset is uniform across the reference peak and its
  isotope, which points to instrument calibration at acquisition time
  rather than the record model; this has not been proven.
- PXD005960's lock function holds one scan whose dominant ion (m/z 825.1)
  is not a known lock compound, so it is not scored.

## Reproduce

```sh
cargo run -p openwraw --release --example audit_corpus -- /mnt/nas/Data/WRaw
cargo run -p openwraw --release --example audit_corpus -- /mnt/nas/Data/WRaw --lock-only
```

The command exits non-zero if any bundle fails to decode or any lock-mass
function fails, so it currently exits 1 because of the PXD029515,
PXD069628 HC20_CE and PXD001175 lock functions.
