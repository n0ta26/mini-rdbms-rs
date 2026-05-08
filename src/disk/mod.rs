pub mod manager;
pub mod storage_disk;

pub use manager::{DEFAULT_PAGE_SIZE, DiskManager, PageId};
pub use storage_disk::StorageDiskManager;
