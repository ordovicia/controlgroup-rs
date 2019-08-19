use std::{error::Error as StdError, fmt};

/// Result type returned from this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type that can be returned from this crate, in the [`Result::Err`] variant. The lower-level
/// source of this error can be obtained via `source` method.
///
/// [`Result::Err`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Err
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn StdError + Sync + Send + 'static>>,
}

/// Kinds of errors that can occur while operating on cgroups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// Failed to do an I/O operation on a cgroup file system.
    Io,

    /// Failed to parse contents in a cgroup file into a value.
    ///
    /// In a future version, there will be some information attached to this variant.
    Parse,

    /// You passed an invalid argument.
    ///
    /// In a future version, this variant may have some information attached, or be replaced with
    /// more fine-grained variants.
    ///
    /// Note that this crate does not catch all errors caused by an invalid argument. In some cases,
    /// the system (kernel) raises an lower-level error, and this crate returns an `Error` with
    /// other `ErrorKind`, typically `Io`. The lower-level source can be obtained via
    /// `Error::source` method.
    InvalidArgument,

    /// You tried to do something invalid.
    ///
    /// In a future version, this variant may have some information attached, or be replaced with
    /// more fine-grained variants.
    ///
    /// Note that this crate does not catch all errors caused by an invalid operation. In some
    /// cases, the system (kernel) raises an lower-level error, and this crate returns an `Error`
    /// with other `ErrorKind`, typically `Io`. The lower-level source can be obtained via
    /// `Error::source` method.
    InvalidOperation,
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self.source {
            Some(ref x) => Some(&**x),
            None => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            ErrorKind::Io => "Unable to do an I/O operation on a cgroup file system",
            ErrorKind::Parse => "Unable to parse contents in a cgroup file",
            ErrorKind::InvalidArgument => "Invalid argument",
            ErrorKind::InvalidOperation => "The requested operation is invalid",
        })?;

        if let Some(ref source) = self.source {
            write!(f, ": {}", source)?;
        }

        Ok(())
    }
}

impl Error {
    pub(crate) fn new(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }

    pub(crate) fn with_source<E>(kind: ErrorKind, source: E) -> Self
    where
        E: StdError + Sync + Send + 'static,
    {
        Self {
            kind,
            source: Some(Box::new(source)),
        }
    }

    /// Returns the kind of this error.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub(crate) fn io<E>(source: E) -> Self
    where
        E: StdError + Sync + Send + 'static,
    {
        Self::with_source(ErrorKind::Io, source)
    }

    pub(crate) fn parse<E>(source: E) -> Self
    where
        E: StdError + Sync + Send + 'static,
    {
        Self::with_source(ErrorKind::Parse, source)
    }
}

#[cfg(test)]
#[allow(unreachable_code, dead_code)]
fn error_impl_sync_send() {
    let _e: Error = unimplemented!();
    let _: &dyn Sync = &_e;
    let _: &dyn Send = &_e;
}
