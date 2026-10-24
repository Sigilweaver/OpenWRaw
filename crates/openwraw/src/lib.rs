#![cfg_attr(not(test), warn(clippy::unwrap_used, clippy::expect_used))]

pub mod mzml;
pub mod raw;
pub mod reader;

pub(crate) mod bytes;
#[cfg(test)]
pub(crate) mod test_corpus;

pub use reader::{DecodedScan, Encoding, FunctionEntry, Reader};

/// Errors returned by this crate.
///
/// The enum is `#[non_exhaustive]`; match it with a wildcard arm.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Reading a file in the bundle failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A file in the bundle does not match the expected layout.
    #[error("parse error: {0}")]
    Parse(String),
    /// Another error, with the file, function or scan it concerns.
    #[error("{context}: {source}")]
    #[non_exhaustive]
    Context {
        context: String,
        #[source]
        source: Box<Error>,
    },
    /// The `openmassspec-core` mzML writer failed.
    #[error(transparent)]
    Core(#[from] openmassspec_core::Error),
}

impl From<Error> for openmassspec_core::Error {
    /// Wrap the error as [`openmassspec_core::Error::Vendor`], so `?` works
    /// in code that returns `openmassspec_core::Result`. An
    /// [`Error::Core`] is unwrapped rather than boxed again.
    fn from(e: Error) -> Self {
        match e {
            Error::Core(inner) => inner,
            other => openmassspec_core::Error::vendor(other),
        }
    }
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
