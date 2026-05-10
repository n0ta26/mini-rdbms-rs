use crate::access::{AccessError, AccessMethod, AccessResult, PageAccessor, RecordId};
use crate::disk::PageId;
use crate::page::{PageManager, PageType};

pub struct HeapAccessMethod<A, P>
where
    A: PageAccessor,
    P: PageManager,
{
    page_accessor: A,
    page_manager: P,
    page_ids: Vec<PageId>,
}

impl<A, P> HeapAccessMethod<A, P>
where
    A: PageAccessor,
    P: PageManager,
{
    pub fn new(page_accessor: A, page_manager: P) -> Self {
        Self {
            page_accessor,
            page_manager,
            page_ids: Vec::new(),
        }
    }

    pub fn with_pages(page_accessor: A, page_manager: P, page_ids: Vec<PageId>) -> Self {
        Self {
            page_accessor,
            page_manager,
            page_ids,
        }
    }

    pub fn page_ids(&self) -> &[PageId] {
        &self.page_ids
    }

    fn find_writable_page_id(&self, record: &[u8]) -> AccessResult<Option<PageId>> {
        for page_id in &self.page_ids {
            let page = self.page_accessor.read_page(*page_id)?;

            let free_space_size = self.page_manager.free_space_size(&page)?;

            if free_space_size >= record.len() {
                return Ok(Some(*page_id));
            }
        }

        Ok(None)
    }

    fn allocate_table_leaf_page(&mut self) -> AccessResult<PageId> {
        let page_id = self.page_accessor.allocate_page()?;
        let mut page = vec![0_u8; self.page_accessor.page_size()];

        self.page_manager
            .init_page(&mut page, page_id, PageType::TableLeaf)?;

        self.page_accessor.write_page(page_id, &page)?;
        self.page_ids.push(page_id);

        Ok(page_id)
    }

    fn validate_table_leaf_page(&self, page_id: PageId, page: &[u8]) -> AccessResult<()> {
        let page_type = self.page_manager.page_type(page)?;

        if page_type != PageType::TableLeaf {
            return Err(AccessError::InvalidAccessPage {
                page_id,
                message: format!("expected TableLeaf page, but got {:?}", page_type),
            });
        }

        Ok(())
    }
}

impl<A, P> AccessMethod for HeapAccessMethod<A, P>
where
    A: PageAccessor,
    P: PageManager,
{
    fn insert(&mut self, record: &[u8]) -> AccessResult<RecordId> {
        let page_id = match self.find_writable_page_id(record)? {
            Some(page_id) => page_id,
            None => self.allocate_table_leaf_page()?,
        };

        let mut page = self.page_accessor.read_page(page_id)?;
        self.validate_table_leaf_page(page_id, &page)?;

        let slot_id = self.page_manager.insert_record(&mut page, record)?;

        self.page_accessor.write_page(page_id, &page)?;

        Ok(RecordId::new(page_id, slot_id))
    }

    fn read(&self, record_id: RecordId) -> AccessResult<Vec<u8>> {
        let page_id = record_id.page_id();
        let page = self.page_accessor.read_page(page_id)?;

        self.validate_table_leaf_page(page_id, &page)?;

        let record = self
            .page_manager
            .read_record(&page, record_id.slot_id())?
            .to_vec();

        Ok(record)
    }

    fn delete(&mut self, record_id: RecordId) -> AccessResult<()> {
        let page_id = record_id.page_id();
        let mut page = self.page_accessor.read_page(page_id)?;

        self.validate_table_leaf_page(page_id, &page)?;

        self.page_manager
            .delete_record(&mut page, record_id.slot_id())?;

        self.page_accessor.write_page(page_id, &page)?;

        Ok(())
    }
}
