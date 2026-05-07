// Reader for _FUNCnnn.DAT - the binary spectrum data files.
// Spectra are stored contiguously, referenced by offsets from the
// paired .IDX file. Multiple compression schemes are known to exist
// across instrument generations; scheme detection is done per-spectrum.
