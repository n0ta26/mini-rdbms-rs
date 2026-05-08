use crate::disk::{DEFAULT_PAGE_SIZE, DiskManager, PageId};
use crate::storage::{StorageEngine, StorageError, StorageResult};

pub struct StorageDiskManager<S>
where
    S: StorageEngine,
{
    storage: S,
    page_size: usize,
}

impl<S> StorageDiskManager<S>
where
    S: StorageEngine,
{
    pub fn new(storage: S) -> Self {
        Self {
            storage,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }

    pub fn with_page_size(storage: S, page_size: usize) -> StorageResult<Self> {
        if page_size == 0 {
            return Err(StorageError::InvalidArgument {
                message: "page_size must be greater than 0".to_string(),
            });
        }

        Ok(Self { storage, page_size })
    }

    pub fn storage(&self) -> &S {
        &self.storage
    }

    fn page_offset(&self, page_id: PageId) -> StorageResult<u64> {
        page_id
            .checked_mul(self.page_size as u64)
            .ok_or_else(|| StorageError::InvalidArgument {
                message: format!(
                    "page offset overflow: page_id={}, page_size={}",
                    page_id, self.page_size
                ),
            })
    }

    fn validate_page_buffer(&self, len: usize) -> StorageResult<()> {
        if len != self.page_size {
            return Err(StorageError::InvalidArgument {
                message: format!(
                    "page buffer size mismatch: expected {}, actual {}",
                    self.page_size, len
                ),
            });
        }

        Ok(())
    }
}

impl<S> DiskManager for StorageDiskManager<S>
where
    S: StorageEngine,
{
    fn page_size(&self) -> usize {
        self.page_size
    }

    fn read_page(&self, page_id: PageId, buf: &mut [u8]) -> StorageResult<()> {
        self.validate_page_buffer(buf.len())?;

        let offset = self.page_offset(page_id)?;

        self.storage.read_exact_at(offset, buf)
    }

    fn write_page(&self, page_id: PageId, page: &[u8]) -> StorageResult<()> {
        self.validate_page_buffer(page.len())?;

        let offset = self.page_offset(page_id)?;

        self.storage.write_exact_at(offset, page)
    }

    fn allocate_page(&self) -> StorageResult<PageId> {
        let len = self.storage.len()?;
        let page_size = self.page_size as u64;

        if len % page_size != 0 {
            return Err(StorageError::InvalidArgument {
                message: format!(
                    "storage length is not page aligned: len={}, page_size={}",
                    len, page_size
                ),
            });
        }

        let page_id = len / page_size;

        let new_len = len
            .checked_add(page_size)
            .ok_or_else(|| StorageError::InvalidArgument {
                message: format!(
                    "storage length overflow: len={}, page_size={}",
                    len, page_size
                ),
            })?;

        self.storage.truncate(new_len)?;

        Ok(page_id)
    }

    fn page_count(&self) -> StorageResult<u64> {
        let len = self.storage.len()?;
        let page_size = self.page_size as u64;

        if len % page_size != 0 {
            return Err(StorageError::InvalidArgument {
                message: format!(
                    "storage length is not page aligned: len={}, page_size={}",
                    len, page_size
                ),
            });
        }

        Ok(len / page_size)
    }

    fn flush(&self) -> StorageResult<()> {
        self.storage.flush()
    }

    fn sync(&self) -> StorageResult<()> {
        self.storage.sync()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{FileStorage, StorageError};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestFile {
        path: PathBuf,
    }

    impl TestFile {
        fn new(test_name: &str) -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

            let unique_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos();
            let file_name = format!(
                "mini_rdbms_{test_name}_{}_{}_{}.db",
                process::id(),
                timestamp,
                unique_id
            );

            Self {
                path: std::env::temp_dir().join(file_name),
            }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    #[test]
    fn new_uses_default_page_size() {
        let test_file = TestFile::new("new_uses_default_page_size");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk = StorageDiskManager::new(storage);

        assert_eq!(disk.page_size(), DEFAULT_PAGE_SIZE);
        assert_eq!(disk.page_size(), 4096);
    }

    #[test]
    fn with_page_size_can_set_custom_page_size() {
        let test_file = TestFile::new("with_page_size_can_set_custom_page_size");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk = StorageDiskManager::with_page_size(storage, 1024)
            .expect("failed to create disk manager");

        assert_eq!(disk.page_size(), 1024);
    }

    #[test]
    fn with_page_size_rejects_zero_page_size() {
        let test_file = TestFile::new("with_page_size_rejects_zero_page_size");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let result = StorageDiskManager::with_page_size(storage, 0);

        assert!(matches!(result, Err(StorageError::InvalidArgument { .. })));
    }

    #[test]
    fn allocate_page_returns_first_page_id() {
        let test_file = TestFile::new("allocate_page_returns_first_page_id");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk = StorageDiskManager::with_page_size(storage, 128)
            .expect("failed to create disk manager");

        let page_id = disk.allocate_page().expect("failed to allocate page");

        assert_eq!(page_id, 0);
        assert_eq!(disk.page_count().expect("failed to get page count"), 1);
    }

    #[test]
    fn allocate_page_returns_sequential_page_ids() {
        let test_file = TestFile::new("allocate_page_returns_sequential_page_ids");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk = StorageDiskManager::with_page_size(storage, 128)
            .expect("failed to create disk manager");

        let page_id_0 = disk.allocate_page().expect("failed to allocate page 0");
        let page_id_1 = disk.allocate_page().expect("failed to allocate page 1");
        let page_id_2 = disk.allocate_page().expect("failed to allocate page 2");

        assert_eq!(page_id_0, 0);
        assert_eq!(page_id_1, 1);
        assert_eq!(page_id_2, 2);

        assert_eq!(disk.page_count().expect("failed to get page count"), 3);
    }

    #[test]
    fn write_page_and_read_page_round_trip() {
        let test_file = TestFile::new("write_page_and_read_page_round_trip");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk = StorageDiskManager::with_page_size(storage, 128)
            .expect("failed to create disk manager");

        let page_id = disk.allocate_page().expect("failed to allocate page");

        let mut write_page = vec![0_u8; disk.page_size()];
        write_page[0..5].copy_from_slice(b"hello");
        write_page[5..10].copy_from_slice(b"world");

        disk.write_page(page_id, &write_page)
            .expect("failed to write page");

        let mut read_page = vec![0_u8; disk.page_size()];

        disk.read_page(page_id, &mut read_page)
            .expect("failed to read page");

        assert_eq!(read_page, write_page);
    }

    #[test]
    fn write_page_uses_page_id_as_page_offset() {
        let test_file = TestFile::new("write_page_uses_page_id_as_page_offset");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk =
            StorageDiskManager::with_page_size(storage, 16).expect("failed to create disk manager");

        let page_id_0 = disk.allocate_page().expect("failed to allocate page 0");
        let page_id_1 = disk.allocate_page().expect("failed to allocate page 1");

        let mut page_0 = vec![0_u8; disk.page_size()];
        page_0[0..4].copy_from_slice(b"aaaa");

        let mut page_1 = vec![0_u8; disk.page_size()];
        page_1[0..4].copy_from_slice(b"bbbb");

        disk.write_page(page_id_0, &page_0)
            .expect("failed to write page 0");

        disk.write_page(page_id_1, &page_1)
            .expect("failed to write page 1");

        let mut read_page_0 = vec![0_u8; disk.page_size()];
        let mut read_page_1 = vec![0_u8; disk.page_size()];

        disk.read_page(page_id_0, &mut read_page_0)
            .expect("failed to read page 0");

        disk.read_page(page_id_1, &mut read_page_1)
            .expect("failed to read page 1");

        assert_eq!(read_page_0, page_0);
        assert_eq!(read_page_1, page_1);
    }

    #[test]
    fn read_page_rejects_buffer_size_mismatch() {
        let test_file = TestFile::new("read_page_rejects_buffer_size_mismatch");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk = StorageDiskManager::with_page_size(storage, 128)
            .expect("failed to create disk manager");

        disk.allocate_page().expect("failed to allocate page");

        let mut invalid_buffer = vec![0_u8; 64];

        let result = disk.read_page(0, &mut invalid_buffer);

        assert!(matches!(result, Err(StorageError::InvalidArgument { .. })));
    }

    #[test]
    fn write_page_rejects_buffer_size_mismatch() {
        let test_file = TestFile::new("write_page_rejects_buffer_size_mismatch");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk = StorageDiskManager::with_page_size(storage, 128)
            .expect("failed to create disk manager");

        disk.allocate_page().expect("failed to allocate page");

        let invalid_page = vec![0_u8; 64];

        let result = disk.write_page(0, &invalid_page);

        assert!(matches!(result, Err(StorageError::InvalidArgument { .. })));
    }

    #[test]
    fn page_count_returns_zero_for_empty_storage() {
        let test_file = TestFile::new("page_count_returns_zero_for_empty_storage");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk = StorageDiskManager::with_page_size(storage, 128)
            .expect("failed to create disk manager");

        assert_eq!(disk.page_count().expect("failed to get page count"), 0);
    }

    #[test]
    fn flush_succeeds_after_write_page() {
        let test_file = TestFile::new("flush_succeeds_after_write_page");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk = StorageDiskManager::with_page_size(storage, 128)
            .expect("failed to create disk manager");

        let page_id = disk.allocate_page().expect("failed to allocate page");

        let mut page = vec![0_u8; disk.page_size()];
        page[0..5].copy_from_slice(b"flush");

        disk.write_page(page_id, &page)
            .expect("failed to write page");

        disk.flush().expect("failed to flush disk manager");
    }

    #[test]
    fn sync_succeeds_after_write_page() {
        let test_file = TestFile::new("sync_succeeds_after_write_page");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk = StorageDiskManager::with_page_size(storage, 128)
            .expect("failed to create disk manager");

        let page_id = disk.allocate_page().expect("failed to allocate page");

        let mut page = vec![0_u8; disk.page_size()];
        page[0..4].copy_from_slice(b"sync");

        disk.write_page(page_id, &page)
            .expect("failed to write page");

        disk.sync().expect("failed to sync disk manager");
    }
}
