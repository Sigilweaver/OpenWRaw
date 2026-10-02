"""Optional native public LCT regression, without vendor-derived output."""

import math
import os

import openwraw
import pytest


@pytest.mark.skipif(
    not os.environ.get("OPENWRAW_LCT_RAW"),
    reason="requires an original public MTBLS701 or MTBLS13770 LCT bundle",
)
def test_lct_twelve_byte_peaks_reach_python_and_records():
    reader = openwraw.RawReader(os.environ["OPENWRAW_LCT_RAW"])
    assert reader.function_encoding(1) == "e"
    for scan_index in (0, reader.n_scans(1) - 1):
        spectrum = reader.read_spectrum(1, scan_index)
        assert len(spectrum.mz) > 0
        assert len(spectrum.mz) == len(spectrum.intensity)
        assert all(math.isfinite(value) and value > 0 for value in spectrum.mz)
        assert all(math.isfinite(value) and value > 0 for value in spectrum.intensity)
        record = reader.read_record(1, scan_index)
        assert record["mz"] == spectrum.mz
        assert record["intensity"] == spectrum.intensity
