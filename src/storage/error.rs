use std::fmt;

/// This enum represents the various errors that can occur in the storage layer, including I/O errors and unexpected read/write results. The `StorageResult` type is a convenient alias for `Result<T, StorageError>`, allowing for consistent error handling across the storage engine implementations. The `From<std::io::Error>` implementation allows for easy conversion from standard I/O errors to our custom `StorageError` type, while the `Display` implementation provides a human-readable description of the error for debugging and logging purposes.
#[derive(Debug)]
pub enum StorageError {
    /// This variant represents an I/O error that occurred during a storage operation, such as reading from or writing to a file. The underlying `std::io::Error` is included to provide detailed information about the nature of the I/O error.
    IoError(std::io::Error),
    /// This variant represents an unexpected result during a read or write operation, where the number of bytes read or written does not match the expected amount. The `offset` field indicates the position in the storage where the operation was attempted, while the `expected` and `actual` fields indicate the expected and actual number of bytes processed, respectively.
    Unexpected {
        offset: u64,
        expected: usize,
        actual: usize,
    },
    // This variant represents an invalid argument error, which can occur when the input parameters for a storage operation are not valid. The `message` field provides a descriptive error message explaining the nature of the invalid argument.
    InvalidArgument {
        message: String,
    },
}

/// This is a type alias for `Result<T, StorageError>`, providing a convenient way to handle results from storage operations.
pub type StorageResult<T> = Result<T, StorageError>;

/// This implementation allows for easy conversion from `std::io::Error` to `StorageError`, enabling seamless error handling when performing I/O operations in the storage engine.
impl From<std::io::Error> for StorageError {
    fn from(error: std::io::Error) -> Self {
        StorageError::IoError(error)
    }
}

/// This implementation provides a human-readable description of the `StorageError`, which is useful for debugging and logging purposes. It matches on the error variant and formats the output accordingly, including details about the I/O error or the unexpected read/write results.
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
            StorageError::InvalidArgument { message } => {
                write!(f, "Invalid argument: {}", message)
            }
        }
    }
}

/// This implementation allows `StorageError` to be used as a standard error type in Rust, enabling it to be returned from functions that may encounter storage-related errors and to be easily integrated with other error handling mechanisms in Rust.
impl std::error::Error for StorageError {}
