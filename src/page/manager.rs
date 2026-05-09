use crate::disk::PageId;
use crate::page::{PageResult, PageType};

pub type SlotId = u16;

pub trait PageManager {
    fn init_page(&self, page: &mut [u8], page_id: PageId, page_type: PageType) -> PageResult<()>;

    fn page_type(&self, page: &[u8]) -> PageResult<PageType>;

    fn free_space_size(&self, page: &[u8]) -> PageResult<usize>;

    fn insert_record(&self, page: &mut [u8], record: &[u8]) -> PageResult<SlotId>;

    fn read_record<'a>(&self, page: &'a [u8], slot_id: SlotId) -> PageResult<&'a [u8]>;

    fn delete_record(&self, page: &mut [u8], slot_id: SlotId) -> PageResult<()>;
}
