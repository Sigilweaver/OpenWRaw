//! Locate public corpus bundles for corpus-gated tests.
//!
//! `OPENWRAW_CORPUS` names the corpus root. Bundles live at
//! `<root>/<PRIDE accession>/<name>.raw`, for example
//! `<root>/PXD058812/molecular_mass_P15_01.raw`.
//!
//! When a bundle is missing the test is skipped with a message on stderr.
//! With `REQUIRE_CORPUS=1` a missing bundle fails the test instead, so a CI
//! job that fetches data cannot pass by silently skipping.
//!
//! Shared by the unit tests (`crate::test_corpus`) and the integration tests
//! (included with `#[path]`), so it uses only `std`.

use std::path::PathBuf;

/// Environment variable naming the corpus root directory.
pub const CORPUS_ENV: &str = "OPENWRAW_CORPUS";
/// Environment variable that turns a missing bundle into a test failure.
pub const REQUIRE_ENV: &str = "REQUIRE_CORPUS";

/// Return the first of `candidates` (bundle paths relative to the corpus
/// root) that exists. Returns `None` to skip the test, or panics when
/// `REQUIRE_CORPUS=1`.
pub fn bundle(candidates: &[&str]) -> Option<PathBuf> {
    let root = std::env::var_os(CORPUS_ENV).map(PathBuf::from);
    let reason = match &root {
        None => format!("{CORPUS_ENV} is not set"),
        Some(root) => {
            if let Some(found) = candidates
                .iter()
                .map(|rel| root.join(rel))
                .find(|path| path.is_dir())
            {
                return Some(found);
            }
            format!("none of {candidates:?} found under {}", root.display())
        }
    };
    if std::env::var(REQUIRE_ENV).is_ok_and(|v| v == "1") {
        panic!("{REQUIRE_ENV}=1 but corpus data is missing: {reason}");
    }
    eprintln!("skipping corpus test: {reason}");
    None
}
