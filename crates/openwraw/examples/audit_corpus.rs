//! Decode every non-lock-mass scan in a directory tree of Waters RAW bundles.
//!
//! Usage: cargo run -p openwraw --release --example audit_corpus -- /path/to/corpus

use std::io::Write;
use std::path::{Path, PathBuf};

use openwraw::{DecodedSpectrum, Reader};

fn bundles(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut pending = vec![root.to_path_buf()];
    let mut found = Vec::new();
    while let Some(dir) = pending.pop() {
        let mut has_header = false;
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file()
                && entry
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_uppercase()
                    .ends_with("_HEADER.TXT")
            {
                has_header = true;
            }
        }
        if has_header {
            found.push(dir);
        }
    }
    found.sort();
    Ok(found)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("expected a corpus directory")?;
    let paths = bundles(&root)?;
    if paths.is_empty() {
        return Err(format!("no RAW bundles found under {}", root.display()).into());
    }
    let mut complete = 0usize;
    let mut opened_scans = 0usize;
    let mut decoded_scans = 0usize;
    for path in &paths {
        match Reader::open(path) {
            Err(error) => println!("OPEN_ERROR {}: {error}", path.display()),
            Ok(reader) => {
                let total = reader.total_scan_count();
                let mut decoded = 0usize;
                let mut nonempty = 0usize;
                let mut first_error = None;
                for result in reader.iter_spectra() {
                    match result {
                        Ok(scan) => {
                            decoded += 1;
                            let peaks = match scan.spectrum {
                                DecodedSpectrum::Plain(s) => s.mz.len(),
                                DecodedSpectrum::Ims(s) => s.mz.len(),
                            };
                            nonempty += usize::from(peaks > 0);
                        }
                        Err(error) => {
                            first_error.get_or_insert_with(|| error.to_string());
                        }
                    }
                }
                opened_scans += total;
                decoded_scans += decoded;
                if total > 0 && decoded == total {
                    complete += 1;
                }
                println!(
                    "{} scans={total} decoded={decoded} nonempty={nonempty} first_error={}",
                    path.display(),
                    first_error.as_deref().unwrap_or("")
                );
            }
        }
        std::io::stdout().flush()?;
    }
    println!(
        "SUMMARY bundles={} complete={} opened_scans={} decoded_scans={}",
        paths.len(),
        complete,
        opened_scans,
        decoded_scans
    );
    if complete != paths.len() {
        std::process::exit(1);
    }
    Ok(())
}
