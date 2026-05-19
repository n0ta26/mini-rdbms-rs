use crate::access::RecordId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionPlan {
    Insert { record: Vec<u8> },
    SelectById { record_id: RecordId },
    DeleteById { record_id: RecordId },
}
