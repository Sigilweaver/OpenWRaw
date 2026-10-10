---
sidebar_position: 3
---

# Ion mobility (IMS)

OpenWRaw does not decode ion mobility.

SYNAPT acquisitions open and decode like any other bundle: each scan is
returned as calibrated m/z and intensity, the same as for a QTof without a
mobility cell. There is no drift-time output in Rust or Python, mzML output
carries no mobility arrays, and no run declares a mobility array kind.

In the public corpus, SYNAPT functions store 8-byte Encoding D records, and
within every sampled scan m/z never decreases, so a stored scan is not split
into drift-ordered blocks. Where the drift time is recorded is not yet
known; see the [format specification](../format/func-dat) for what has been
established.
