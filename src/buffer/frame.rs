use crate::disk::PageId;

#[derive(Debug, Clone)]
pub struct BufferFrame {
    page_id: PageId,
    data: Vec<u8>,
    pin_count: usize,
    is_dirty: bool,
}

impl BufferFrame {
    pub fn new(page_id: PageId, data: Vec<u8>) -> Self {
        Self {
            page_id,
            data,
            pin_count: 0,
            is_dirty: false,
        }
    }

    pub fn page_id(&self) -> PageId {
        self.page_id
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn set_data(&mut self, data: Vec<u8>) {
        self.data = data;
        self.is_dirty = true;
    }

    pub fn pin(&mut self) {
        self.pin_count += 1;
    }

    pub fn unpin(&mut self) {
        self.pin_count -= 1;
    }

    pub fn pin_count(&self) -> usize {
        self.pin_count
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn set_dirty(&mut self, is_dirty: bool) {
        self.is_dirty = is_dirty;
    }

    pub fn is_evictable(&self) -> bool {
        self.pin_count == 0
    }
}
