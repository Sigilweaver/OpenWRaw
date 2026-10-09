"""Regress the public one-channel descriptor layout through the Python API."""

import shutil
import struct

import openwraw


def test_one_channel_descriptor_header_opens_and_maps_chro001(raw_bundle, tmp_path):
    bundle = tmp_path / "one-channel.raw"
    shutil.copytree(raw_bundle, bundle)

    # Native public PSU and MSV000083877 files have these two field descriptors
    # inside the header. A single channel after it makes a 213-byte file.
    header = bytearray(128)
    struct.pack_into("<4H", header, 0, 128, 1, 85, 2)
    struct.pack_into("<3H", header, 32, 1, 2, 0)
    header[38:43] = b"Flags"
    struct.pack_into("<H", header, 64, 4)
    struct.pack_into("<3H", header, 80, 2, 5, 4)
    header[86:97] = b"Description"
    struct.pack_into("<H", header, 112, 81)
    channel = bytearray(85)
    struct.pack_into("<I", channel, 0, 4)
    payload = b"Pressure\0$CC$,1.0,3,0,0,psi\0"
    channel[4 : 4 + len(payload)] = payload
    metadata = header + channel
    assert len(metadata) == 213
    (bundle / "_CHROMS.INF").write_bytes(metadata)

    data = bytearray(128)
    struct.pack_into("<4H", data, 0, 128, 1, 8, 2)
    data.extend(struct.pack("<4f", 0.0, 450.0, 0.5, 500.0))
    (bundle / "_CHRO001.DAT").write_bytes(data)

    reader = openwraw.RawReader(str(bundle))
    assert [(ch.index, ch.name, ch.units) for ch in reader.channels] == [
        (0, "Pressure", "psi")
    ]
    assert [(p.rt_min, p.value) for p in reader.read_chrom(0)] == [
        (0.0, 450.0),
        (0.5, 500.0),
    ]
    chrom = reader.read_chromatograms()[0]
    assert chrom["time_sec"] == [0.0, 30.0]
    assert chrom["intensity"] == [450.0, 500.0]
