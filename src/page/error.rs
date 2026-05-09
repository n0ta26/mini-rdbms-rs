#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageError {
    InvalidPageSize { expected_min: usize, actual: usize },

    InvalidMagic { actual: u16 },

    InvalidPageType { actual: u8 },

    NotEnoughSpace { required: usize, available: usize },

    InvalidSlotId { slot_id: u16, slot_count: u16 },

    DeletedSlot { slot_id: u16 },

    CorruptedPage { message: String },
}

pub type PageResult<T> = Result<T, PageError>;
