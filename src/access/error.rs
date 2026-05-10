use crate::disk::PageId;
use crate::page::PageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessError {
    Page(PageError),

    PageNotFound { page_id: PageId },

    NoWritablePage,

    InvalidAccessPage { page_id: PageId, message: String },
}

pub type AccessResult<T> = Result<T, AccessError>;

impl From<PageError> for AccessError {
    fn from(error: PageError) -> Self {
        Self::Page(error)
    }
}
