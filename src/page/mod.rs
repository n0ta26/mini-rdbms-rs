mod error;
mod manager;
mod slotted_page;

pub use error::{PageError, PageResult};
pub use manager::{PageManager, SlotId};
pub use slotted_page::{PageType, SlottedPageManager};
