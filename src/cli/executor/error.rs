use crate::access::AccessError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutorError {
    Access(AccessError),
}

pub type ExecutorResult<T> = Result<T, ExecutorError>;

impl From<AccessError> for ExecutorError {
    fn from(error: AccessError) -> Self {
        Self::Access(error)
    }
}
