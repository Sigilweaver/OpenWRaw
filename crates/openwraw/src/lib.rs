#![cfg_attr(not(test), warn(clippy::unwrap_used, clippy::expect_used))]

pub mod mzml;
pub mod raw;
pub mod reader;

pub(crate) mod bytes;
#[cfg(test)]
pub(crate) mod test_corpus;

pub use reader::{DecodedScan, Encoding, FunctionEntry, Reader};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("{context}: {source}")]
    Context {
        context: String,
        #[source]
        source: Box<Error>,
    },
}

impl Error {
    pub(crate) fn with_context(self, context: impl Into<String>) -> Self {
        Self::Context {
            context: context.into(),
            source: Box::new(self),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
