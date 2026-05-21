use crate::buffer::{BufferError, BufferFrame, BufferManager, BufferResult};
use crate::disk::{DiskManager, PageId};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

pub struct LruBufferManager<D>
where
    D: DiskManager,
{
    disk_manager: D,
    inner: Mutex<LruBufferManagerInner>,
    capacity: usize,
    page_size: usize,
}

#[derive(Debug)]
struct LruBufferManagerInner {
    page_table: HashMap<PageId, BufferFrame>,
    lru_list: VecDeque<PageId>,
}

impl<D> LruBufferManager<D>
where
    D: DiskManager,
{
    pub fn new(disk_manager: D, capacity: usize) -> BufferResult<Self> {
        if capacity == 0 {
            return Err(BufferError::InvalidCapacity);
        }

        let page_size = disk_manager.page_size();

        Ok(Self {
            disk_manager,
            inner: Mutex::new(LruBufferManagerInner {
                page_table: HashMap::new(),
                lru_list: VecDeque::new(),
            }),
            capacity,
            page_size,
        })
    }

    pub fn disk_manager(&self) -> &D {
        &self.disk_manager
    }

    fn validate_page_size(&self, data: &[u8]) -> BufferResult<()> {
        if data.len() != self.page_size {
            return Err(BufferError::InvalidPageSize {
                expected: self.page_size,
                actual: data.len(),
            });
        }

        Ok(())
    }

    fn touch_lru(inner: &mut LruBufferManagerInner, page_id: PageId) {
        inner.lru_list.retain(|id| *id != page_id);
        inner.lru_list.push_back(page_id);
    }

    fn find_victim(inner: &mut LruBufferManagerInner) -> Option<PageId> {
        let lru_len = inner.lru_list.len();

        for _ in 0..lru_len {
            let page_id = inner.lru_list.pop_front()?;

            let is_evictable = inner
                .page_table
                .get(&page_id)
                .map(BufferFrame::is_evictable)
                .unwrap_or(false);

            if is_evictable {
                return Some(page_id);
            }

            inner.lru_list.push_back(page_id);
        }

        None
    }

    fn evict_if_needed(&self, inner: &mut LruBufferManagerInner) -> BufferResult<()> {
        if inner.page_table.len() < self.capacity {
            return Ok(());
        }

        let victim_page_id = Self::find_victim(inner).ok_or(BufferError::NoEvictablePage)?;

        let victim = inner
            .page_table
            .remove(&victim_page_id)
            .ok_or(BufferError::PageNotFound {
                page_id: victim_page_id,
            })?;

        if victim.is_dirty() {
            self.disk_manager
                .write_page(victim.page_id(), victim.data())?;
        }

        Ok(())
    }

    fn insert_page(
        &self,
        inner: &mut LruBufferManagerInner,
        page_id: PageId,
        data: Vec<u8>,
        is_dirty: bool,
    ) -> BufferResult<()> {
        self.evict_if_needed(inner)?;

        let mut frame = BufferFrame::new(page_id, data);
        frame.pin();
        frame.set_dirty(is_dirty);

        inner.page_table.insert(page_id, frame);
        Self::touch_lru(inner, page_id);

        Ok(())
    }
}

impl<D> BufferManager for LruBufferManager<D>
where
    D: DiskManager,
{
    fn fetch_page(&self, page_id: PageId) -> BufferResult<Vec<u8>> {
        let mut inner = self
            .inner
            .lock()
            .expect("failed to lock lru buffer manager");

        if let Some(frame) = inner.page_table.get_mut(&page_id) {
            frame.pin();
            let data = frame.data().to_vec();
            Self::touch_lru(&mut inner, page_id);
            return Ok(data);
        }

        let mut data = vec![0_u8; self.page_size];
        self.disk_manager.read_page(page_id, &mut data)?;

        self.insert_page(&mut inner, page_id, data.clone(), false)?;

        Ok(data)
    }

    fn new_page(&self) -> BufferResult<(PageId, Vec<u8>)> {
        let page_id = self.disk_manager.allocate_page()?;
        let data = vec![0_u8; self.page_size];

        let mut inner = self
            .inner
            .lock()
            .expect("failed to lock lru buffer manager");

        self.insert_page(&mut inner, page_id, data.clone(), true)?;

        Ok((page_id, data))
    }

    fn update_page(&self, page_id: PageId, data: Vec<u8>) -> BufferResult<()> {
        self.validate_page_size(&data)?;

        let mut inner = self
            .inner
            .lock()
            .expect("failed to lock lru buffer manager");

        if let Some(frame) = inner.page_table.get_mut(&page_id) {
            frame.set_data(data);
            Self::touch_lru(&mut inner, page_id);
            return Ok(());
        }

        self.insert_page(&mut inner, page_id, data, true)
    }

    fn unpin_page(&self, page_id: PageId, is_dirty: bool) -> BufferResult<()> {
        let mut inner = self
            .inner
            .lock()
            .expect("failed to lock lru buffer manager");

        let frame = inner
            .page_table
            .get_mut(&page_id)
            .ok_or(BufferError::PageNotFound { page_id })?;

        if frame.pin_count() == 0 {
            return Err(BufferError::PageNotPinned { page_id });
        }

        frame.unpin();

        if is_dirty {
            frame.set_dirty(true);
        }

        Ok(())
    }

    fn flush_page(&self, page_id: PageId) -> BufferResult<()> {
        let mut inner = self
            .inner
            .lock()
            .expect("failed to lock lru buffer manager");

        let frame = inner
            .page_table
            .get_mut(&page_id)
            .ok_or(BufferError::PageNotFound { page_id })?;

        if frame.is_dirty() {
            self.disk_manager.write_page(page_id, frame.data())?;
            frame.set_dirty(false);
        }

        Ok(())
    }

    fn flush_all(&self) -> BufferResult<()> {
        let mut inner = self
            .inner
            .lock()
            .expect("failed to lock lru buffer manager");

        for (page_id, frame) in inner.page_table.iter_mut() {
            if frame.is_dirty() {
                self.disk_manager.write_page(*page_id, frame.data())?;
                frame.set_dirty(false);
            }
        }

        Ok(())
    }

    fn len(&self) -> usize {
        let inner = self
            .inner
            .lock()
            .expect("failed to lock lru buffer manager");

        inner.page_table.len()
    }

    fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::BufferError;
    use crate::disk::StorageDiskManager;
    use crate::storage::FileStorage;
    use crate::test_utils::TestFile;

    fn create_buffer_manager(
        test_name: &str,
        page_size: usize,
        capacity: usize,
    ) -> (TestFile, LruBufferManager<StorageDiskManager<FileStorage>>) {
        let test_file = TestFile::new(test_name);

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk_manager = StorageDiskManager::with_page_size(storage, page_size)
            .expect("failed to create disk manager");

        let buffer_manager =
            LruBufferManager::new(disk_manager, capacity).expect("failed to create buffer manager");

        (test_file, buffer_manager)
    }

    #[test]
    fn new_rejects_zero_capacity() {
        let test_file = TestFile::new("new_rejects_zero_capacity");

        let storage = FileStorage::open(test_file.path()).expect("failed to open file storage");

        let disk_manager = StorageDiskManager::with_page_size(storage, 128)
            .expect("failed to create disk manager");

        let result = LruBufferManager::new(disk_manager, 0);

        assert!(matches!(result, Err(BufferError::InvalidCapacity)));
    }

    #[test]
    fn new_page_allocates_page_and_caches_it() {
        let (_test_file, buffer_manager) =
            create_buffer_manager("new_page_allocates_page_and_caches_it", 128, 2);

        let (page_id, page) = buffer_manager
            .new_page()
            .expect("failed to allocate new page");

        assert_eq!(page_id, 0);
        assert_eq!(page.len(), 128);
        assert_eq!(page, vec![0_u8; 128]);
        assert_eq!(buffer_manager.len(), 1);
        assert_eq!(buffer_manager.capacity(), 2);
        assert!(!buffer_manager.is_empty());
    }

    #[test]
    fn new_page_returns_sequential_page_ids() {
        let (_test_file, buffer_manager) =
            create_buffer_manager("new_page_returns_sequential_page_ids", 128, 3);

        let (page_id_0, _) = buffer_manager
            .new_page()
            .expect("failed to allocate page 0");

        buffer_manager
            .unpin_page(page_id_0, false)
            .expect("failed to unpin page 0");

        let (page_id_1, _) = buffer_manager
            .new_page()
            .expect("failed to allocate page 1");

        buffer_manager
            .unpin_page(page_id_1, false)
            .expect("failed to unpin page 1");

        let (page_id_2, _) = buffer_manager
            .new_page()
            .expect("failed to allocate page 2");

        assert_eq!(page_id_0, 0);
        assert_eq!(page_id_1, 1);
        assert_eq!(page_id_2, 2);
        assert_eq!(buffer_manager.len(), 3);
    }

    #[test]
    fn fetch_page_reads_existing_page_from_disk_and_caches_it() {
        let (_test_file, buffer_manager) = create_buffer_manager(
            "fetch_page_reads_existing_page_from_disk_and_caches_it",
            128,
            2,
        );

        let page_id = buffer_manager
            .disk_manager()
            .allocate_page()
            .expect("failed to allocate page");

        let mut expected_page = vec![0_u8; 128];
        expected_page[0..5].copy_from_slice(b"hello");

        buffer_manager
            .disk_manager()
            .write_page(page_id, &expected_page)
            .expect("failed to write page directly");

        let fetched_page = buffer_manager
            .fetch_page(page_id)
            .expect("failed to fetch page");

        assert_eq!(fetched_page, expected_page);
        assert_eq!(buffer_manager.len(), 1);

        buffer_manager
            .unpin_page(page_id, false)
            .expect("failed to unpin page");
    }

    #[test]
    fn update_page_marks_cached_page_as_dirty_and_flush_page_persists_it() {
        let (_test_file, buffer_manager) = create_buffer_manager(
            "update_page_marks_cached_page_as_dirty_and_flush_page_persists_it",
            128,
            2,
        );

        let (page_id, mut page) = buffer_manager
            .new_page()
            .expect("failed to allocate new page");

        page[0..5].copy_from_slice(b"dirty");

        buffer_manager
            .update_page(page_id, page.clone())
            .expect("failed to update page");

        buffer_manager
            .unpin_page(page_id, true)
            .expect("failed to unpin dirty page");

        buffer_manager
            .flush_page(page_id)
            .expect("failed to flush page");

        let mut read_back = vec![0_u8; 128];

        buffer_manager
            .disk_manager()
            .read_page(page_id, &mut read_back)
            .expect("failed to read page from disk");

        assert_eq!(read_back, page);
    }

    #[test]
    fn flush_all_persists_all_dirty_pages() {
        let (_test_file, buffer_manager) =
            create_buffer_manager("flush_all_persists_all_dirty_pages", 128, 3);

        let (page_id_0, mut page_0) = buffer_manager
            .new_page()
            .expect("failed to allocate page 0");

        buffer_manager
            .unpin_page(page_id_0, false)
            .expect("failed to unpin page 0");

        let (page_id_1, mut page_1) = buffer_manager
            .new_page()
            .expect("failed to allocate page 1");

        page_0[0..4].copy_from_slice(b"aaaa");
        page_1[0..4].copy_from_slice(b"bbbb");

        buffer_manager
            .update_page(page_id_0, page_0.clone())
            .expect("failed to update page 0");

        buffer_manager
            .update_page(page_id_1, page_1.clone())
            .expect("failed to update page 1");

        buffer_manager
            .unpin_page(page_id_1, true)
            .expect("failed to unpin page 1");

        buffer_manager
            .flush_all()
            .expect("failed to flush all pages");

        let mut read_page_0 = vec![0_u8; 128];
        let mut read_page_1 = vec![0_u8; 128];

        buffer_manager
            .disk_manager()
            .read_page(page_id_0, &mut read_page_0)
            .expect("failed to read page 0");

        buffer_manager
            .disk_manager()
            .read_page(page_id_1, &mut read_page_1)
            .expect("failed to read page 1");

        assert_eq!(read_page_0, page_0);
        assert_eq!(read_page_1, page_1);
    }

    #[test]
    fn update_page_rejects_invalid_page_size() {
        let (_test_file, buffer_manager) =
            create_buffer_manager("update_page_rejects_invalid_page_size", 128, 2);

        let (page_id, _) = buffer_manager
            .new_page()
            .expect("failed to allocate new page");

        let invalid_page = vec![0_u8; 64];

        let result = buffer_manager.update_page(page_id, invalid_page);

        assert!(matches!(
            result,
            Err(BufferError::InvalidPageSize {
                expected: 128,
                actual: 64,
            })
        ));
    }

    #[test]
    fn unpin_page_rejects_unknown_page() {
        let (_test_file, buffer_manager) =
            create_buffer_manager("unpin_page_rejects_unknown_page", 128, 2);

        let result = buffer_manager.unpin_page(999, false);

        assert!(matches!(
            result,
            Err(BufferError::PageNotFound { page_id: 999 })
        ));
    }

    #[test]
    fn unpin_page_rejects_page_that_is_not_pinned() {
        let (_test_file, buffer_manager) =
            create_buffer_manager("unpin_page_rejects_page_that_is_not_pinned", 128, 2);

        let (page_id, _) = buffer_manager
            .new_page()
            .expect("failed to allocate new page");

        buffer_manager
            .unpin_page(page_id, false)
            .expect("failed to unpin page");

        let result = buffer_manager.unpin_page(page_id, false);

        assert!(matches!(
            result,
            Err(BufferError::PageNotPinned { page_id: actual_page_id })
                if actual_page_id == page_id
        ));
    }

    #[test]
    fn pinned_page_is_not_evicted() {
        let (_test_file, buffer_manager) =
            create_buffer_manager("pinned_page_is_not_evicted", 128, 1);

        let (_page_id, _) = buffer_manager
            .new_page()
            .expect("failed to allocate first page");

        let result = buffer_manager.new_page();

        assert!(matches!(result, Err(BufferError::NoEvictablePage)));
    }

    #[test]
    fn evicts_unpinned_page_when_capacity_is_full() {
        let (_test_file, buffer_manager) =
            create_buffer_manager("evicts_unpinned_page_when_capacity_is_full", 128, 1);

        let (page_id_0, _) = buffer_manager
            .new_page()
            .expect("failed to allocate page 0");

        buffer_manager
            .unpin_page(page_id_0, false)
            .expect("failed to unpin page 0");

        let (page_id_1, _) = buffer_manager
            .new_page()
            .expect("failed to allocate page 1");

        assert_eq!(page_id_0, 0);
        assert_eq!(page_id_1, 1);
        assert_eq!(buffer_manager.len(), 1);

        buffer_manager
            .unpin_page(page_id_1, false)
            .expect("failed to unpin page 1");

        let fetched_page_0 = buffer_manager
            .fetch_page(page_id_0)
            .expect("failed to fetch evicted page 0 again");

        assert_eq!(fetched_page_0, vec![0_u8; 128]);
        assert_eq!(buffer_manager.len(), 1);
    }

    #[test]
    fn dirty_page_is_written_back_before_eviction() {
        let (_test_file, buffer_manager) =
            create_buffer_manager("dirty_page_is_written_back_before_eviction", 128, 1);

        let (page_id_0, mut page_0) = buffer_manager
            .new_page()
            .expect("failed to allocate page 0");

        page_0[0..5].copy_from_slice(b"evict");

        buffer_manager
            .update_page(page_id_0, page_0.clone())
            .expect("failed to update page 0");

        buffer_manager
            .unpin_page(page_id_0, true)
            .expect("failed to unpin page 0");

        let (page_id_1, _) = buffer_manager
            .new_page()
            .expect("failed to allocate page 1");

        assert_eq!(page_id_1, 1);
        assert_eq!(buffer_manager.len(), 1);

        let mut read_back = vec![0_u8; 128];

        buffer_manager
            .disk_manager()
            .read_page(page_id_0, &mut read_back)
            .expect("failed to read evicted page 0");

        assert_eq!(read_back, page_0);
    }

    #[test]
    fn flush_page_rejects_unknown_page() {
        let (_test_file, buffer_manager) =
            create_buffer_manager("flush_page_rejects_unknown_page", 128, 2);

        let result = buffer_manager.flush_page(999);

        assert!(matches!(
            result,
            Err(BufferError::PageNotFound { page_id: 999 })
        ));
    }
}
