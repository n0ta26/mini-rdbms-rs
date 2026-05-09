use crate::disk::PageId;
use crate::storage::StorageError;
use std::error::Error;
use std::fmt;

pub type BufferResult<T> = Result<T, BufferError>;

#[derive(Debug)]
pub enum BufferError {
    InvalidCapacity,

    InvalidPageSize { expected: usize, actual: usize },

    NoEvictablePage,

    PageNotFound { page_id: PageId },

    PageNotPinned { page_id: PageId },

    Storage(StorageError),
}

impl From<StorageError> for BufferError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl fmt::Display for BufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BufferError::InvalidCapacity => {
                write!(f, "buffer pool capacity must be greater than zero")
            }
            BufferError::InvalidPageSize { expected, actual } => {
                write!(
                    f,
                    "invalid page size: expected {expected} bytes, actual {actual} bytes"
                )
            }
            BufferError::NoEvictablePage => {
                write!(f, "no evictable page found")
            }
            BufferError::PageNotFound { page_id } => {
                write!(f, "page not found in buffer pool: page_id={page_id}")
            }
            BufferError::PageNotPinned { page_id } => {
                write!(f, "page is not pinned: page_id={page_id}")
            }
            BufferError::Storage(error) => {
                write!(f, "storage error in buffer layer: {error}")
            }
        }
    }
}

impl Error for BufferError {}
