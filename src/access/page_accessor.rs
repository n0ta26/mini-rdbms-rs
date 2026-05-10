use crate::access::AccessResult;
use crate::disk::PageId;

pub trait PageAccessor {
    fn allocate_page(&mut self) -> AccessResult<PageId>;

    fn read_page(&self, page_id: PageId) -> AccessResult<Vec<u8>>;

    fn write_page(&mut self, page_id: PageId, page: &[u8]) -> AccessResult<()>;

    fn page_size(&self) -> usize;
}
