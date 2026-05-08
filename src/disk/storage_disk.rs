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
