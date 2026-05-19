use crate::access::RecordId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionOutput {
    Inserted { record_id: RecordId },
    Selected { record: Vec<u8> },
    Deleted { affected_rows: usize },
}
