use std::fmt;

#[derive(Debug)]
pub enum StorageError {
    IoError(std::io::Error),
    Unexpected {
        offset: u64,
        expected: usize,
        actual: usize,
    },
}

pub type StorageResult<T> = Result<T, StorageError>;

impl From<std::io::Error> for StorageError {
    fn from(error: std::io::Error) -> Self {
        StorageError::IoError(error)
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::IoError(err) => write!(f, "Storage I/O error: {}", err),
            StorageError::Unexpected {
                offset,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Unexpected data at offset {}: expected {} bytes, got {} bytes",
                    offset, expected, actual
                )
            }
        }
    }
}

impl std::error::Error for StorageError {}
