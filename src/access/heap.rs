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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::{DEFAULT_PAGE_SIZE, PageId};
    use crate::page::{PageError, SlottedPageManager};
    use std::collections::HashMap;

    #[derive(Debug)]
    struct InMemoryPageAccessor {
        pages: HashMap<PageId, Vec<u8>>,
        next_page_id: PageId,
        page_size: usize,
    }

    impl InMemoryPageAccessor {
        fn new(page_size: usize) -> Self {
            Self {
                pages: HashMap::new(),
                next_page_id: 0,
                page_size,
            }
        }
    }

    impl PageAccessor for InMemoryPageAccessor {
        fn allocate_page(&mut self) -> AccessResult<PageId> {
            let page_id = self.next_page_id;
            self.next_page_id += 1;

            self.pages.insert(page_id, vec![0_u8; self.page_size]);

            Ok(page_id)
        }

        fn read_page(&self, page_id: PageId) -> AccessResult<Vec<u8>> {
            self.pages
                .get(&page_id)
                .cloned()
                .ok_or(AccessError::PageNotFound { page_id })
        }

        fn write_page(&mut self, page_id: PageId, page: &[u8]) -> AccessResult<()> {
            if !self.pages.contains_key(&page_id) {
                return Err(AccessError::PageNotFound { page_id });
            }

            self.pages.insert(page_id, page.to_vec());

            Ok(())
        }

        fn page_size(&self) -> usize {
            self.page_size
        }
    }

    fn new_heap_access_method() -> HeapAccessMethod<InMemoryPageAccessor, SlottedPageManager> {
        HeapAccessMethod::new(
            InMemoryPageAccessor::new(DEFAULT_PAGE_SIZE),
            SlottedPageManager::new(),
        )
    }

    #[test]
    fn insert_stores_record_and_returns_record_id() {
        let mut access_method = new_heap_access_method();

        let record_id = access_method
            .insert(b"hello")
            .expect("failed to insert record");

        assert_eq!(record_id.page_id(), 0);
        assert_eq!(record_id.slot_id(), 0);
        assert_eq!(access_method.page_ids(), &[0]);
    }

    #[test]
    fn read_returns_inserted_record() {
        let mut access_method = new_heap_access_method();

        let record_id = access_method
            .insert(b"hello")
            .expect("failed to insert record");

        let record = access_method
            .read(record_id)
            .expect("failed to read record");

        assert_eq!(record, b"hello");
    }

    #[test]
    fn insert_multiple_records_into_same_page() {
        let mut access_method = new_heap_access_method();

        let first_record_id = access_method
            .insert(b"first")
            .expect("failed to insert first record");

        let second_record_id = access_method
            .insert(b"second")
            .expect("failed to insert second record");

        assert_eq!(first_record_id.page_id(), second_record_id.page_id());
        assert_ne!(first_record_id.slot_id(), second_record_id.slot_id());

        assert_eq!(
            access_method
                .read(first_record_id)
                .expect("failed to read first record"),
            b"first"
        );

        assert_eq!(
            access_method
                .read(second_record_id)
                .expect("failed to read second record"),
            b"second"
        );
    }

    #[test]
    fn delete_marks_record_as_deleted() {
        let mut access_method = new_heap_access_method();

        let record_id = access_method
            .insert(b"hello")
            .expect("failed to insert record");

        access_method
            .delete(record_id)
            .expect("failed to delete record");

        let result = access_method.read(record_id);

        assert!(matches!(
            result,
            Err(AccessError::Page(PageError::DeletedSlot { slot_id: 0 }))
        ));
    }

    #[test]
    fn read_returns_page_not_found_when_page_does_not_exist() {
        let access_method = new_heap_access_method();
        let record_id = RecordId::new(999, 0);

        let result = access_method.read(record_id);

        assert!(matches!(
            result,
            Err(AccessError::PageNotFound { page_id: 999 })
        ));
    }

    #[test]
    fn insert_allocates_new_page_when_current_page_has_no_space() {
        let mut access_method =
            HeapAccessMethod::new(InMemoryPageAccessor::new(64), SlottedPageManager::new());

        access_method
            .insert(&[1_u8; 20])
            .expect("failed to insert first record");

        access_method
            .insert(&[2_u8; 20])
            .expect("failed to insert second record");

        assert!(access_method.page_ids().len() >= 2);
    }
}
