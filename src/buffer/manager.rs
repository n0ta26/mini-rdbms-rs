use crate::buffer::BufferResult;
use crate::disk::PageId;

pub trait BufferManager {
    fn fetch_page(&self, page_id: PageId) -> BufferResult<Vec<u8>>;

    fn new_page(&self) -> BufferResult<(PageId, Vec<u8>)>;

    fn update_page(&self, page_id: PageId, data: Vec<u8>) -> BufferResult<()>;

    fn unpin_page(&self, page_id: PageId, is_dirty: bool) -> BufferResult<()>;

    fn flush_page(&self, page_id: PageId) -> BufferResult<()>;

    fn flush_all(&self) -> BufferResult<()>;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn capacity(&self) -> usize;
}
