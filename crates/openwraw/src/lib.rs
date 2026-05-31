#![cfg_attr(not(test), warn(clippy::unwrap_used, clippy::expect_used))]

pub mod mzml;
pub mod raw;
pub mod reader;

pub(crate) mod bytes;

pub use reader::{DecodedScan, DecodedSpectrum, Encoding, FunctionEntry, Reader};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, Error>;
