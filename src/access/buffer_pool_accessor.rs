use crate::access::{AccessError, AccessResult, PageAccessor};
use crate::buffer::{BufferError, BufferManager};
use crate::disk::PageId;

pub struct BufferPoolPageAccessor<B>
where
    B: BufferManager,
{
    buffer_manager: B,
    page_size: usize,
}

impl<B> BufferPoolPageAccessor<B>
where
    B: BufferManager,
{
    pub fn new(buffer_manager: B, page_size: usize) -> Self {
        Self {
            buffer_manager,
            page_size,
        }
    }

    fn map_buffer_error(error: BufferError, page_id: Option<PageId>) -> AccessError {
        match error {
            BufferError::PageNotFound { page_id } => AccessError::PageNotFound { page_id },
            error => AccessError::InvalidAccessPage {
                page_id: page_id.unwrap_or(0),
                message: error.to_string(),
            },
        }
    }
}

impl<B> PageAccessor for BufferPoolPageAccessor<B>
where
    B: BufferManager,
{
    fn allocate_page(&mut self) -> AccessResult<PageId> {
        let (page_id, _) = self
            .buffer_manager
            .new_page()
            .map_err(|error| Self::map_buffer_error(error, None))?;

        self.buffer_manager
            .unpin_page(page_id, false)
            .map_err(|error| Self::map_buffer_error(error, Some(page_id)))?;

        Ok(page_id)
    }

    fn read_page(&self, page_id: PageId) -> AccessResult<Vec<u8>> {
        let page = self
            .buffer_manager
            .fetch_page(page_id)
            .map_err(|error| Self::map_buffer_error(error, Some(page_id)))?;

        self.buffer_manager
            .unpin_page(page_id, false)
            .map_err(|error| Self::map_buffer_error(error, Some(page_id)))?;

        Ok(page)
    }

    fn write_page(&mut self, page_id: PageId, page: &[u8]) -> AccessResult<()> {
        self.buffer_manager
            .fetch_page(page_id)
            .map_err(|error| Self::map_buffer_error(error, Some(page_id)))?;

        self.buffer_manager
            .update_page(page_id, page.to_vec())
            .map_err(|error| Self::map_buffer_error(error, Some(page_id)))?;

        self.buffer_manager
            .unpin_page(page_id, true)
            .map_err(|error| Self::map_buffer_error(error, Some(page_id)))?;

        self.buffer_manager
            .flush_page(page_id)
            .map_err(|error| Self::map_buffer_error(error, Some(page_id)))?;

        Ok(())
    }

    fn page_size(&self) -> usize {
        self.page_size
    }
}
