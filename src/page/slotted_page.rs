use crate::disk::PageId;
use crate::page::{PageError, PageManager, PageResult, SlotId};

const PAGE_MAGIC: u16 = 0xDB01;

const INVALID_PAGE_ID: u64 = u64::MAX;

const PAGE_HEADER_SIZE: usize = 25;

const SLOT_SIZE: usize = 5;

const SLOT_FLAG_USED: u8 = 1;

const SLOT_FLAG_DELETED: u8 = 2;

#[derive(Debug, Default, Clone, Copy)]
pub struct SlottedPageManager;

impl SlottedPageManager {
    pub fn new() -> Self {
        Self
    }

    fn validate_page_size(&self, page: &[u8]) -> PageResult<()> {
        let min_size = PAGE_HEADER_SIZE + SLOT_SIZE;

        if page.len() < min_size {
            return Err(PageError::InvalidPageSize {
                expected_min: min_size,
                actual: page.len(),
            });
        }

        if page.len() > u16::MAX as usize {
            return Err(PageError::InvalidPageSize {
                expected_min: 0,
                actual: page.len(),
            });
        }

        Ok(())
    }

    fn read_header(&self, page: &[u8]) -> PageResult<PageHeader> {
        self.validate_page_size(page)?;

        let magic = read_u16(page, 0);
        if magic != PAGE_MAGIC {
            return Err(PageError::InvalidMagic { actual: magic });
        }

        let page_type_raw = page[10];
        let page_type = PageType::try_from(page_type_raw)?;

        let header = PageHeader {
            page_id: read_u64(page, 2),
            page_type,
            slot_count: read_u16(page, 11),
            free_start: read_u16(page, 13),
            free_end: read_u16(page, 15),
            next_page_id: decode_page_id(read_u64(page, 17)),
        };

        self.validate_header(page, &header)?;

        Ok(header)
    }

    fn write_header(&self, page: &mut [u8], header: &PageHeader) -> PageResult<()> {
        self.validate_page_size(page)?;
        self.validate_header(page, header)?;

        write_u16(page, 0, PAGE_MAGIC);
        write_u64(page, 2, header.page_id);
        page[10] = header.page_type.into();
        write_u16(page, 11, header.slot_count);
        write_u16(page, 13, header.free_start);
        write_u16(page, 15, header.free_end);
        write_u64(page, 17, encode_page_id(header.next_page_id));

        Ok(())
    }

    fn validate_header(&self, page: &[u8], header: &PageHeader) -> PageResult<()> {
        let page_len = page.len();

        if header.free_start as usize > header.free_end as usize {
            return Err(PageError::CorruptedPage {
                message: format!(
                    "free_start must be <= free_end: free_start={}, free_end={}",
                    header.free_start, header.free_end
                ),
            });
        }

        if header.free_end as usize > page_len {
            return Err(PageError::CorruptedPage {
                message: format!(
                    "free_end must be <= page size: free_end={}, page_size={}",
                    header.free_end, page_len
                ),
            });
        }

        let expected_free_start = PAGE_HEADER_SIZE + header.slot_count as usize * SLOT_SIZE;

        if header.free_start as usize != expected_free_start {
            return Err(PageError::CorruptedPage {
                message: format!(
                    "invalid free_start: expected={}, actual={}",
                    expected_free_start, header.free_start
                ),
            });
        }

        Ok(())
    }

    fn slot_offset(&self, slot_id: SlotId) -> usize {
        PAGE_HEADER_SIZE + slot_id as usize * SLOT_SIZE
    }

    fn read_slot(&self, page: &[u8], slot_id: SlotId) -> PageResult<Slot> {
        let header = self.read_header(page)?;

        if slot_id >= header.slot_count {
            return Err(PageError::InvalidSlotId {
                slot_id,
                slot_count: header.slot_count,
            });
        }

        let offset = self.slot_offset(slot_id);

        Ok(Slot {
            offset: read_u16(page, offset),
            len: read_u16(page, offset + 2),
            flags: page[offset + 4],
        })
    }

    fn write_slot(&self, page: &mut [u8], slot_id: SlotId, slot: &Slot) -> PageResult<()> {
        let header = self.read_header(page)?;

        if slot_id >= header.slot_count {
            return Err(PageError::InvalidSlotId {
                slot_id,
                slot_count: header.slot_count,
            });
        }

        let offset = self.slot_offset(slot_id);

        write_u16(page, offset, slot.offset);
        write_u16(page, offset + 2, slot.len);
        page[offset + 4] = slot.flags;

        Ok(())
    }
}

impl PageManager for SlottedPageManager {
    fn init_page(&self, page: &mut [u8], page_id: PageId, page_type: PageType) -> PageResult<()> {
        self.validate_page_size(page)?;

        page.fill(0);

        let header = PageHeader {
            page_id,
            page_type,
            slot_count: 0,
            free_start: PAGE_HEADER_SIZE as u16,
            free_end: page.len() as u16,
            next_page_id: None,
        };

        self.write_header(page, &header)
    }

    fn page_type(&self, page: &[u8]) -> PageResult<PageType> {
        Ok(self.read_header(page)?.page_type)
    }

    fn free_space_size(&self, page: &[u8]) -> PageResult<usize> {
        let header = self.read_header(page)?;

        Ok(header.free_end as usize - header.free_start as usize)
    }

    fn insert_record(&self, page: &mut [u8], record: &[u8]) -> PageResult<SlotId> {
        self.validate_page_size(page)?;

        if record.len() > u16::MAX as usize {
            return Err(PageError::NotEnoughSpace {
                required: record.len(),
                available: self.free_space_size(page)?,
            });
        }

        let mut header = self.read_header(page)?;

        let required = SLOT_SIZE + record.len();
        let available = self.free_space_size(page)?;

        if required > available {
            return Err(PageError::NotEnoughSpace {
                required,
                available,
            });
        }

        let slot_id = header.slot_count;

        // レコード本体はページ末尾側から前に詰めます。
        let record_start = header.free_end as usize - record.len();
        let record_end = header.free_end as usize;

        page[record_start..record_end].copy_from_slice(record);

        header.slot_count += 1;
        header.free_start += SLOT_SIZE as u16;
        header.free_end = record_start as u16;

        self.write_header(page, &header)?;

        let slot = Slot {
            offset: record_start as u16,
            len: record.len() as u16,
            flags: SLOT_FLAG_USED,
        };

        self.write_slot(page, slot_id, &slot)?;

        Ok(slot_id)
    }

    fn read_record<'a>(&self, page: &'a [u8], slot_id: SlotId) -> PageResult<&'a [u8]> {
        let slot = self.read_slot(page, slot_id)?;

        if slot.flags == SLOT_FLAG_DELETED {
            return Err(PageError::DeletedSlot { slot_id });
        }

        if slot.flags != SLOT_FLAG_USED {
            return Err(PageError::CorruptedPage {
                message: format!("invalid slot flags: {}", slot.flags),
            });
        }

        let start = slot.offset as usize;
        let end = start + slot.len as usize;

        if end > page.len() {
            return Err(PageError::CorruptedPage {
                message: format!(
                    "record range is out of page: start={}, end={}, page_size={}",
                    start,
                    end,
                    page.len()
                ),
            });
        }

        Ok(&page[start..end])
    }

    fn delete_record(&self, page: &mut [u8], slot_id: SlotId) -> PageResult<()> {
        let mut slot = self.read_slot(page, slot_id)?;

        if slot.flags == SLOT_FLAG_DELETED {
            return Err(PageError::DeletedSlot { slot_id });
        }

        slot.flags = SLOT_FLAG_DELETED;

        self.write_slot(page, slot_id, &slot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageType {
    Free = 0,
    TableLeaf = 1,
    TableOverflow = 2,
    IndexInternal = 3,
    IndexLeaf = 4,
}

impl TryFrom<u8> for PageType {
    type Error = PageError;

    fn try_from(value: u8) -> PageResult<Self> {
        match value {
            0 => Ok(PageType::Free),
            1 => Ok(PageType::TableLeaf),
            2 => Ok(PageType::TableOverflow),
            3 => Ok(PageType::IndexInternal),
            4 => Ok(PageType::IndexLeaf),
            actual => Err(PageError::InvalidPageType { actual }),
        }
    }
}

impl From<PageType> for u8 {
    fn from(value: PageType) -> Self {
        match value {
            PageType::Free => 0,
            PageType::TableLeaf => 1,
            PageType::TableOverflow => 2,
            PageType::IndexInternal => 3,
            PageType::IndexLeaf => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PageHeader {
    page_id: PageId,
    page_type: PageType,
    slot_count: u16,
    free_start: u16,
    free_end: u16,
    next_page_id: Option<PageId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    offset: u16,
    len: u16,
    flags: u8,
}

fn encode_page_id(page_id: Option<PageId>) -> u64 {
    page_id.unwrap_or(INVALID_PAGE_ID)
}

fn decode_page_id(value: u64) -> Option<PageId> {
    if value == INVALID_PAGE_ID {
        None
    } else {
        Some(value)
    }
}

fn read_u16(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn read_u64(buf: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
        buf[offset + 4],
        buf[offset + 5],
        buf[offset + 6],
        buf[offset + 7],
    ])
}

fn write_u64(buf: &mut [u8], offset: usize, value: u64) {
    buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
