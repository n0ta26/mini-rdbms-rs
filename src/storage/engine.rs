use crate::storage::error::StorageResult;

/// This trait abstracts over the underlying storage mechanism, allowing for different implementations (e.g., in-memory, file-based, etc.) while providing a consistent interface for reading and writing data at specific offsets.
pub trait StorageEngine {
    /// This method should return an error if the end of the storage is reached before filling the buffer.
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> StorageResult<()>;

    /// This method should return an error if the end of the storage is reached before writing all bytes from the buffer.
    fn write_exact_at(&self, offset: u64, buf: &[u8]) -> StorageResult<()>;

    /// This method should ensure that all buffered data is flushed to the underlying storage device, but it does not guarantee that the data is physically written to the device (i.e., it may still be in the OS buffer).
    fn flush(&self) -> StorageResult<()>;

    /// This method should ensure that all buffered data is flushed to the underlying storage device and that the device has physically written the data to its media. This is typically achieved by calling `fsync` on a file descriptor in a file-based storage implementation.
    fn sync(&self) -> StorageResult<()>;

    /// This method should return the current size of the storage in bytes. For a file-based storage implementation, this would typically involve querying the file size. For an in-memory storage implementation, this would involve tracking the size of the allocated buffer.
    fn len(&self) -> StorageResult<u64>;

    /// This method should be able to both shrink and extend the storage. When shrinking, any data beyond the new size should be discarded. When extending, the new space should be filled with zeros.
    fn truncate(&self, size: u64) -> StorageResult<()>;

    /// This method should return `true` if the storage contains no data (i.e., its length is zero) and `false` otherwise. For a file-based storage implementation, this would typically involve checking if the file size is zero. For an in-memory storage implementation, this would involve checking if the allocated buffer is empty.
    fn is_empty(&self) -> StorageResult<bool> {
        Ok(self.len()? == 0)
    }
}
