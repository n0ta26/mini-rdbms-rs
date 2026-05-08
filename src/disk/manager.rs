use crate::storage::StorageResult;

pub const DEFAULT_PAGE_SIZE: usize = 4096;

pub type PageId = u64;

pub trait DiskManager {
    fn page_size(&self) -> usize;

    fn read_page(&self, page_id: PageId, buf: &mut [u8]) -> StorageResult<()>;

    fn write_page(&self, page_id: PageId, page: &[u8]) -> StorageResult<()>;

    fn allocate_page(&self) -> StorageResult<PageId>;

    fn page_count(&self) -> StorageResult<u64>;

    fn flush(&self) -> StorageResult<()>;

    fn sync(&self) -> StorageResult<()>;
}
