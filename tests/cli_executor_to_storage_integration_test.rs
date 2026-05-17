use mini_rdbms_rs::access::{AccessError, BufferPoolPageAccessor, HeapAccessMethod, RecordId};
use mini_rdbms_rs::buffer::LruBufferManager;
use mini_rdbms_rs::cli::executor::{
    AccessMethodExecutor, ExecutionOutput, ExecutionPlan, Executor, ExecutorError,
};
use mini_rdbms_rs::disk::{PageId, StorageDiskManager};
use mini_rdbms_rs::page::{PageError, SlottedPageManager};
use mini_rdbms_rs::storage::FileStorage;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

type TestDiskManager = StorageDiskManager<FileStorage>;
type TestBufferManager = LruBufferManager<TestDiskManager>;
type TestAccessMethod =
    HeapAccessMethod<BufferPoolPageAccessor<TestBufferManager>, SlottedPageManager>;
type TestExecutor = AccessMethodExecutor<TestAccessMethod>;

struct TestFile {
    path: PathBuf,
}

impl TestFile {
    fn new(test_name: &str) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        let unique_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let file_name = format!(
            "mini_rdbms_{test_name}_{}_{}_{}.db",
            process::id(),
            timestamp,
            unique_id
        );

        Self {
            path: std::env::temp_dir().join(file_name),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn create_executor(
    path: &Path,
    page_size: usize,
    capacity: usize,
    page_ids: Vec<PageId>,
) -> TestExecutor {
    let storage = FileStorage::open(path).expect("failed to open file storage");
    let disk_manager = StorageDiskManager::with_page_size(storage, page_size)
        .expect("failed to create disk manager");
    let buffer_manager =
        LruBufferManager::new(disk_manager, capacity).expect("failed to create buffer manager");
    let page_accessor = BufferPoolPageAccessor::new(buffer_manager, page_size);
    let access_method =
        HeapAccessMethod::with_pages(page_accessor, SlottedPageManager::new(), page_ids);

    AccessMethodExecutor::new(access_method)
}

#[test]
fn executor_round_trip_through_storage_stack() {
    let test_file = TestFile::new("executor_round_trip_through_storage_stack");
    let mut executor = create_executor(test_file.path(), 256, 8, vec![]);

    let insert_result = executor
        .execute(ExecutionPlan::Insert {
            record: b"hello-from-cli-executor".to_vec(),
        })
        .expect("failed to execute insert");

    let record_id = match insert_result {
        ExecutionOutput::Inserted { record_id } => record_id,
        _ => panic!("expected inserted output"),
    };

    let select_result = executor
        .execute(ExecutionPlan::SelectById { record_id })
        .expect("failed to execute select");

    assert_eq!(
        select_result,
        ExecutionOutput::Selected {
            record: b"hello-from-cli-executor".to_vec()
        }
    );

    let delete_result = executor
        .execute(ExecutionPlan::DeleteById { record_id })
        .expect("failed to execute delete");

    assert_eq!(delete_result, ExecutionOutput::Deleted { affected_rows: 1 });

    let select_after_delete = executor.execute(ExecutionPlan::SelectById { record_id });

    assert!(
        matches!(
            select_after_delete,
            Err(ExecutorError::Access(AccessError::Page(PageError::DeletedSlot { slot_id }))) if slot_id == record_id.slot_id()
        ),
        "deleted record should return DeletedSlot error"
    );
}

#[test]
fn executor_returns_invalid_access_page_for_unknown_record_id() {
    let test_file = TestFile::new("executor_returns_invalid_access_page_for_unknown_record_id");
    let mut executor = create_executor(test_file.path(), 256, 8, vec![]);

    let result = executor.execute(ExecutionPlan::SelectById {
        record_id: RecordId::new(999, 0),
    });

    assert!(matches!(
        result,
        Err(ExecutorError::Access(AccessError::InvalidAccessPage { page_id: 999, .. }))
    ));
}
