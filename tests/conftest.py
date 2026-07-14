"""Shared fixtures for the openwraw Python test suite.

Downloads and caches the same small Waters .raw.zip bundle that
ci.yml's validate-mzml job uses (PXD058812), so Rust and Python CI
exercise one canonical fixture.
"""

import os
import shutil
import urllib.request
import zipfile
from pathlib import Path

import pytest

WATERS_ZIP_URL = os.environ.get(
    "WATERS_ZIP_URL",
    "https://ftp.pride.ebi.ac.uk/pride/data/archive/2025/05/PXD058812/"
    "ITEM_M11_7_His_tag_01.raw.zip",
)
WATERS_BUNDLE = os.environ.get("WATERS_BUNDLE", "ITEM_M11_7_His_tag_01.raw")

CACHE_DIR = Path(__file__).parent / ".cache"


@pytest.fixture(scope="session")
def raw_bundle():
    """Path to a small Waters .raw bundle.

    Set OPENWRAW_TEST_BUNDLE to point at an already-extracted bundle
    (CI does this after downloading it once for the whole workflow).
    Otherwise downloads and caches WATERS_ZIP_URL locally. Skips the
    session rather than failing when no bundle can be obtained.
    """
    existing = os.environ.get("OPENWRAW_TEST_BUNDLE")
    if existing:
        path = Path(existing)
        if not path.is_dir():
            pytest.skip(f"OPENWRAW_TEST_BUNDLE={existing} is not a directory")
        return path

    bundle_dir = CACHE_DIR / WATERS_BUNDLE
    if bundle_dir.is_dir():
        return bundle_dir

    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    zip_path = CACHE_DIR / "waters.raw.zip"
    try:
        req = urllib.request.Request(
            WATERS_ZIP_URL, headers={"User-Agent": "OpenWRaw-CI/1.0"}
        )
        with urllib.request.urlopen(req, timeout=60) as resp, open(
            zip_path, "wb"
        ) as f:
            shutil.copyfileobj(resp, f)
        with zipfile.ZipFile(zip_path) as zf:
            zf.extractall(CACHE_DIR)
    except OSError as e:
        pytest.skip(f"could not download Waters test bundle: {e}")
    finally:
        zip_path.unlink(missing_ok=True)

    if not bundle_dir.is_dir():
        pytest.skip(f"expected bundle dir {bundle_dir} missing after extraction")
    return bundle_dir
