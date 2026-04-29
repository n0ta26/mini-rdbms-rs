use crate::storage::error::StorageResult;

pub trait StorageEngine {
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> StorageResult<()>;

    fn write_exact_at(&self, offset: u64, buf: &[u8]) -> StorageResult<()>;

    fn flush(&self) -> StorageResult<()>;

    fn sync(&self) -> StorageResult<()>;

    fn len(&self) -> StorageResult<u64>;

    fn truncate(&self, size: u64) -> StorageResult<()>;

    fn is_empty(&self) -> StorageResult<bool> {
        Ok(self.len()? == 0)
    }
}
