use kaitai::KError;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("{0:?}")]
    ReadError(KError),
    #[error("Seek to {0} failed: {1:?}")]
    SeekError(usize, KError)
}

#[derive(Debug, thiserror::Error)]
pub enum DescriptorError {
    #[error("failed to parse '{0}' as a u64")]
    U64ParseError(String),
    #[error("failed to parse '{0}' as Kind enum")]
    KindParseError(String),
    #[error("")]
    ParseExtentDescriptionError,
    #[error("failed to recognize descriptor")]
    UnrecognizedDescriptor
}

#[derive(Debug, thiserror::Error)]
#[error("Error while deserializing {0} struct: {1:?}")]
pub struct DeserializationError(pub &'static str, pub KError);

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("Failed to start tokio Runtime: {0}")]
    TokioRuntimeFailed(std::io::Error),
    #[error("{0}")]
    CacheSetupFailed(std::io::Error)
}

#[derive(Debug, thiserror::Error)]
pub enum OpenErrorKind {
    #[error("{0}")]
    IoError(#[from] IoError),
    #[error("Expected size of parent extent descriptor {0}, actual {1}")]
    BadParentExtentDescriptorSize(u64, u64),
    #[error("Error reading descriptor: {0}")]
    DescriptorError(#[from] DescriptorError),
    #[error("{0}")]
    DeserializationFailed(#[from] DeserializationError),
    #[error("No KDMV or COWD headers detected")]
    InvalidFileHeader,
    #[error("{0}")]
    InitializationFailed(#[from] InitError),
    #[error("Malformed path or URL: {0}")]
    BadPath(String),
    #[error("Unsupported URL scheme: {0}")]
    UnsupportedScheme(String)
}

impl From<KError> for OpenErrorKind {
    fn from(e: KError) -> Self {
        Self::IoError(IoError::ReadError(e))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{path}: {kind}")]
pub struct OpenError {
    pub path: PathBuf,
    #[source]
    pub kind: OpenErrorKind
}

impl From<OpenErrorKind> for OpenError {
    fn from(e: OpenErrorKind) -> Self {
        Self {
            path: "".into(), // set using with_path()
            kind: e
        }
    }
}

impl From<KError> for OpenError {
    fn from(e: KError) -> Self {
        Self {
            path: "".into(), // set using with_path()
            kind: OpenErrorKind::IoError(IoError::ReadError(e))
        }
    }
}

impl From<DescriptorError> for OpenError {
    fn from(e: DescriptorError) -> Self {
        Self {
            path: "".into(), // set using with_path()
            kind: OpenErrorKind::DescriptorError(e)
        }
    }
}

impl From<DeserializationError> for OpenError {
    fn from(e: DeserializationError) -> Self {
        Self {
            path: "".into(), // set using with_path()
            kind: OpenErrorKind::DeserializationFailed(e)
        }
    }
}

impl From<std::io::Error> for OpenError {
    fn from(e: std::io::Error) -> Self {
        Self {
            path: "".into(), // set using with_path()
            kind: OpenErrorKind::IoError(IoError::IoError(e))
        }
    }
}

impl From<IoError> for OpenError {
    fn from(e: IoError) -> Self {
        Self {
            path: "".into(), // set using with_path()
            kind: OpenErrorKind::IoError(e)
        }
    }
}

impl OpenError {
    pub fn with_path<T: AsRef<Path>>(self, path: T) -> Self {
        Self {
            path: path.as_ref().into(),
            kind: self.kind
        }
    }
}
