"""Corpus regression for DDA precursor m/z and MSe MS level.

Needs the public PRIDE bundles under OPENWRAW_CORPUS; skipped otherwise,
and failed instead when REQUIRE_CORPUS=1.
"""

import os
from pathlib import Path

import openwraw
import pytest


def _bundle(rel: str) -> Path:
    root = os.environ.get("OPENWRAW_CORPUS")
    path = Path(root) / rel if root else None
    if path is not None and path.is_dir():
        return path
    if os.environ.get("REQUIRE_CORPUS") == "1":
        pytest.fail(f"REQUIRE_CORPUS=1 but {rel} is missing")
    pytest.skip(f"corpus bundle {rel} not available")


def test_dda_precursor_comes_from_sts_not_zero_set_mass():
    path = _bundle("PXD041695/JLG-1D-ISY-C18_Q0764.raw")
    r = openwraw.RawReader(str(path))
    assert r.instrument_parameters()["functions"][2]["set_mass_da"] is None
    assert r.ms_level(1) == 1
    assert r.ms_level(2) == 2
    assert r.scan_parameters(1, 0)["precursor_mz"] is None
    mz = r.scan_parameters(2, 0)["precursor_mz"]
    assert mz == pytest.approx(823.7629, abs=5e-4)
    record = r.read_record(2, 0)
    assert record["ms_level"] == 2
    assert record["precursor"]["target_mz"] == pytest.approx(mz)
    assert record["precursor"]["selected_mz"] == pytest.approx(mz)


def test_mse_high_energy_tof_ms_function_is_ms2():
    path = _bundle("PXD009047/110810_UCA114_CD_JN018_02.raw")
    r = openwraw.RawReader(str(path))
    assert r.ms_level(1) == 1
    assert r.ms_level(2) == 2
    assert r.read_record(2, 0)["ms_level"] == 2


def test_collision_energies_list_with_unit_from_the_section_label():
    # DDA function with one collision cell and no ramp; the section labels its
    # collision energy "(eV)".
    path = _bundle("PXD041695/JLG-1D-ISY-C18_Q0764.raw")
    r = openwraw.RawReader(str(path))
    precursor = r.read_record(2, 0)["precursor"]
    assert "collision_energy" not in precursor
    assert "ce_is_nce" not in precursor
    energies = precursor["collision_energies"]
    assert len(energies) == 1
    assert energies[0]["unit"] == "ev"
    assert energies[0]["value"] == pytest.approx(
        r.scan_parameters(2, 0)["collision_energy_ev"]
    )


def test_collision_energy_unit_is_none_when_the_section_does_not_label_it():
    path = _bundle("PXD010569/20161025_AV_Dario_Banda_10.raw")
    r = openwraw.RawReader(str(path))
    energies = r.read_record(2, 0)["precursor"]["collision_energies"]
    assert len(energies) == 1
    assert energies[0]["unit"] is None


def test_ramped_function_states_no_collision_energy():
    # The high-energy function ramps 15 to 40 eV; the STS channel holds 15.0.
    path = _bundle("PXD075602/DHPR_11257-1.raw")
    r = openwraw.RawReader(str(path))
    assert r.scan_parameters(2, 0)["collision_energy_ev"] == 15.0
    record = r.read_record(2, 0)
    assert record["ms_level"] == 2
    assert "collision_energies" not in record["precursor"]
    assert "ramp 15 to 40" in record["extra"]["openwraw.collision_energy_unstated"]
