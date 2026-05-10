use crate::access::{AccessResult, RecordId};

pub trait AccessMethod {
    fn insert(&mut self, record: &[u8]) -> AccessResult<RecordId>;

    fn read(&self, record_id: RecordId) -> AccessResult<Vec<u8>>;

    fn delete(&mut self, record_id: RecordId) -> AccessResult<()>;
}
