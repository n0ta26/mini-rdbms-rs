use crate::access::AccessMethod;
use crate::cli::executor::{ExecutionOutput, ExecutionPlan, ExecutorResult};

pub trait Executor {
    fn execute(&mut self, plan: ExecutionPlan) -> ExecutorResult<ExecutionOutput>;
}

pub struct AccessMethodExecutor<A>
where
    A: AccessMethod,
{
    access_method: A,
}

impl<A> AccessMethodExecutor<A>
where
    A: AccessMethod,
{
    pub fn new(access_method: A) -> Self {
        Self { access_method }
    }

    pub fn access_method(&self) -> &A {
        &self.access_method
    }

    pub fn access_method_mut(&mut self) -> &mut A {
        &mut self.access_method
    }
}

impl<A> Executor for AccessMethodExecutor<A>
where
    A: AccessMethod,
{
    fn execute(&mut self, plan: ExecutionPlan) -> ExecutorResult<ExecutionOutput> {
        match plan {
            ExecutionPlan::Insert { record } => {
                let record_id = self.access_method.insert(&record)?;
                Ok(ExecutionOutput::Inserted { record_id })
            }
            ExecutionPlan::SelectById { record_id } => {
                let record = self.access_method.read(record_id)?;
                Ok(ExecutionOutput::Selected { record })
            }
            ExecutionPlan::DeleteById { record_id } => {
                self.access_method.delete(record_id)?;
                Ok(ExecutionOutput::Deleted { affected_rows: 1 })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::{AccessError, RecordId};

    #[derive(Debug, Default)]
    struct MockAccessMethod {
        next_slot_id: u16,
        records: Vec<(RecordId, Vec<u8>)>,
    }

    impl MockAccessMethod {
        fn new() -> Self {
            Self::default()
        }
    }

    impl AccessMethod for MockAccessMethod {
        fn insert(&mut self, record: &[u8]) -> crate::access::AccessResult<RecordId> {
            let record_id = RecordId::new(0, self.next_slot_id);
            self.next_slot_id += 1;

            self.records.push((record_id, record.to_vec()));

            Ok(record_id)
        }

        fn read(&self, record_id: RecordId) -> crate::access::AccessResult<Vec<u8>> {
            self.records
                .iter()
                .find(|(id, _)| *id == record_id)
                .map(|(_, record)| record.clone())
                .ok_or(AccessError::PageNotFound {
                    page_id: record_id.page_id(),
                })
        }

        fn delete(&mut self, record_id: RecordId) -> crate::access::AccessResult<()> {
            let before = self.records.len();
            self.records.retain(|(id, _)| *id != record_id);

            if self.records.len() == before {
                return Err(AccessError::PageNotFound {
                    page_id: record_id.page_id(),
                });
            }

            Ok(())
        }
    }

    #[test]
    fn execute_insert_returns_inserted_record_id() {
        let mut executor = AccessMethodExecutor::new(MockAccessMethod::new());

        let result = executor
            .execute(ExecutionPlan::Insert {
                record: b"hello".to_vec(),
            })
            .expect("failed to execute insert plan");

        assert_eq!(
            result,
            ExecutionOutput::Inserted {
                record_id: RecordId::new(0, 0)
            }
        );
    }

    #[test]
    fn execute_select_by_id_returns_record() {
        let mut executor = AccessMethodExecutor::new(MockAccessMethod::new());

        let inserted = executor
            .execute(ExecutionPlan::Insert {
                record: b"hello".to_vec(),
            })
            .expect("failed to execute insert plan");

        let record_id = match inserted {
            ExecutionOutput::Inserted { record_id } => record_id,
            _ => panic!("expected inserted output"),
        };

        let result = executor
            .execute(ExecutionPlan::SelectById { record_id })
            .expect("failed to execute select plan");

        assert_eq!(
            result,
            ExecutionOutput::Selected {
                record: b"hello".to_vec()
            }
        );
    }

    #[test]
    fn execute_delete_by_id_removes_record() {
        let mut executor = AccessMethodExecutor::new(MockAccessMethod::new());

        let inserted = executor
            .execute(ExecutionPlan::Insert {
                record: b"hello".to_vec(),
            })
            .expect("failed to execute insert plan");

        let record_id = match inserted {
            ExecutionOutput::Inserted { record_id } => record_id,
            _ => panic!("expected inserted output"),
        };

        let delete_result = executor
            .execute(ExecutionPlan::DeleteById { record_id })
            .expect("failed to execute delete plan");

        assert_eq!(delete_result, ExecutionOutput::Deleted { affected_rows: 1 });

        let select_result = executor.execute(ExecutionPlan::SelectById { record_id });

        assert!(matches!(
            select_result,
            Err(crate::cli::executor::ExecutorError::Access(
                AccessError::PageNotFound { page_id: 0 }
            ))
        ));
    }

    #[test]
    fn execute_propagates_access_error() {
        let mut executor = AccessMethodExecutor::new(MockAccessMethod::new());

        let result = executor.execute(ExecutionPlan::SelectById {
            record_id: RecordId::new(999, 0),
        });

        assert!(matches!(
            result,
            Err(crate::cli::executor::ExecutorError::Access(
                AccessError::PageNotFound { page_id: 999 }
            ))
        ));
    }
}
