"""Smoke tests for the openwraw Python bindings.

Exercises every method/property on RawReader against a real Waters
bundle, asserting on shape/type/non-emptiness rather than exact
values - matching exact values would mean validating against
vendor-derived expected output, which CONTRIBUTING.md's clean-room
policy rules out.
"""

import pytest

import openwraw


def test_header(raw_bundle):
    r = openwraw.RawReader(str(raw_bundle))
    header = r.header
    for field in (
        header.version,
        header.acquired_name,
        header.acquired_date,
        header.acquired_time,
        header.instrument,
        header.operator,
        header.sample_description,
    ):
        assert field is None or isinstance(field, str)
    assert "RunHeader(" in repr(header)


def test_polarity(raw_bundle):
    r = openwraw.RawReader(str(raw_bundle))
    assert r.polarity in ("positive", "negative", None)


def test_functions(raw_bundle):
    r = openwraw.RawReader(str(raw_bundle))
    functions = r.functions
    assert len(functions) > 0
    for f in functions:
        assert f.index >= 1
        assert isinstance(f.function_type, int)
        assert isinstance(f.scan_subtype, int)
        assert isinstance(f.cycle_time_s, float)
        assert isinstance(f.interscan_delay_s, float)
        assert isinstance(f.scan_time_s, float)
        assert isinstance(f.tof_depth, int)
        assert f.mz_high >= f.mz_low
        assert isinstance(f.is_lock_mass, bool)
        assert "FunctionInfo(" in repr(f)


def test_channels(raw_bundle):
    r = openwraw.RawReader(str(raw_bundle))
    channels = r.channels
    if not channels:
        pytest.skip("bundle has no _CHROMS.INF (no channels)")
    for ch in channels:
        assert isinstance(ch.index, int)
        assert isinstance(ch.source_type, int)
        assert isinstance(ch.name, str)
        assert isinstance(ch.scale_f, float)
        assert isinstance(ch.units, str)
        assert "ChromChannel(" in repr(ch)


def test_ms_level_and_encoding(raw_bundle):
    r = openwraw.RawReader(str(raw_bundle))
    for f in r.functions:
        assert r.ms_level(f.index) in (1, 2)
        assert r.function_encoding(f.index) in ("a", "b")


def test_n_scans_and_retention_time(raw_bundle):
    r = openwraw.RawReader(str(raw_bundle))
    for f in r.functions:
        n = r.n_scans(f.index)
        assert n > 0
        rt = r.retention_time(f.index, 0)
        assert rt >= 0.0


def test_read_spectrum(raw_bundle):
    r = openwraw.RawReader(str(raw_bundle))
    for f in r.functions:
        spec = r.read_spectrum(f.index, 0)
        assert len(spec.mz) == len(spec.intensity)
        assert len(spec) == len(spec.mz)
        assert len(spec) > 0
        assert "Spectrum(" in repr(spec)


def test_read_ims_spectrum(raw_bundle):
    r = openwraw.RawReader(str(raw_bundle))
    saw_ims_function = False
    for f in r.functions:
        if r.function_encoding(f.index) != "b":
            continue
        saw_ims_function = True
        ims = r.read_ims_spectrum(f.index, 0)
        assert len(ims.mz) == len(ims.drift_time_ms) == len(ims.intensity)
        assert len(ims) == len(ims.mz)
        assert len(ims) > 0
        assert "ImsSpectrum(" in repr(ims)
    if not saw_ims_function:
        pytest.skip("bundle has no Encoding B (IMS) function")


def test_read_chrom(raw_bundle):
    r = openwraw.RawReader(str(raw_bundle))
    channels = r.channels
    if not channels:
        pytest.skip("bundle has no _CHROMS.INF (no channels)")
    for ch in channels:
        points = r.read_chrom(ch.index)
        assert len(points) > 0
        for p in points:
            assert isinstance(p.rt_min, float)
            assert isinstance(p.value, float)
            assert "ChromPoint(" in repr(p)


def test_repr(raw_bundle):
    r = openwraw.RawReader(str(raw_bundle))
    assert "RawReader(" in repr(r)
