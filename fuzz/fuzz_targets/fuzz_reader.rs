#![no_main]

use libfuzzer_sys::fuzz_target;
use std::io::Write as _;

/// Splits `data` into exactly `n` chunks using u16-LE length prefixes, so a
/// single fuzzer-supplied byte string can populate every file a `.raw/`
/// bundle needs at once. Each length is clamped to the bytes actually
/// remaining, so this never panics regardless of input.
fn split_chunks(mut data: &[u8], n: usize) -> Vec<&[u8]> {
    let mut chunks = Vec::with_capacity(n);
    for _ in 0..n {
        if data.len() < 2 {
            chunks.push(&data[..0]);
            continue;
        }
        let want = u16::from_le_bytes([data[0], data[1]]) as usize;
        data = &data[2..];
        let take = want.min(data.len());
        let (chunk, rest) = data.split_at(take);
        chunks.push(chunk);
        data = rest;
    }
    chunks
}

fuzz_target!(|data: &[u8]| {
    let chunks = split_chunks(data, 5);
    let header_txt = chunks[0];
    let extern_inf = chunks[1];
    let functions_inf = chunks[2];
    let idx = chunks[3];
    let dat = chunks[4];

    let Ok(dir) = tempfile::tempdir() else {
        return;
    };
    let path = dir.path();

    let write = |name: &str, bytes: &[u8]| -> std::io::Result<()> {
        std::fs::File::create(path.join(name))?.write_all(bytes)
    };
    if write("_HEADER.TXT", header_txt).is_err()
        || write("_extern.inf", extern_inf).is_err()
        || write("_FUNCTNS.INF", functions_inf).is_err()
        || write("_FUNC001.IDX", idx).is_err()
        || write("_FUNC001.DAT", dat).is_err()
    {
        return;
    }

    // The entry point every caller (Rust and Python) goes through: opening
    // a bundle and decoding every scan must never panic, abort, or attempt
    // an allocation disproportionate to the bytes actually on disk, no
    // matter how malformed or adversarial the file contents are.
    if let Ok(reader) = openwraw::Reader::open(path) {
        for scan in reader.iter_spectra() {
            let _ = scan;
        }
    }
});
