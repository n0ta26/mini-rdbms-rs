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
