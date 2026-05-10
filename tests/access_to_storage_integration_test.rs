use mini_rdbms_rs::access::{AccessError, AccessMethod, BufferPoolPageAccessor, HeapAccessMethod};
use mini_rdbms_rs::buffer::LruBufferManager;
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

fn create_access_method(
    path: &Path,
    page_size: usize,
    capacity: usize,
    page_ids: Vec<PageId>,
) -> TestAccessMethod {
    let storage = FileStorage::open(path).expect("failed to open file storage");
    let disk_manager = StorageDiskManager::with_page_size(storage, page_size)
        .expect("failed to create disk manager");
    let buffer_manager =
        LruBufferManager::new(disk_manager, capacity).expect("failed to create buffer manager");
    let page_accessor = BufferPoolPageAccessor::new(buffer_manager, page_size);

    HeapAccessMethod::with_pages(page_accessor, SlottedPageManager::new(), page_ids)
}

#[test]
fn heap_access_method_round_trip_through_storage_stack() {
    let test_file = TestFile::new("heap_access_method_round_trip_through_storage_stack");
    let mut access_method = create_access_method(test_file.path(), 256, 8, vec![]);

    let record_id = access_method
        .insert(b"hello-from-access-method")
        .expect("failed to insert record");

    let record = access_method
        .read(record_id)
        .expect("failed to read record");
    assert_eq!(record, b"hello-from-access-method");

    access_method
        .delete(record_id)
        .expect("failed to delete record");

    let result = access_method.read(record_id);
    assert!(
        matches!(
            result,
            Err(AccessError::Page(PageError::DeletedSlot { slot_id })) if slot_id == record_id.slot_id()
        ),
        "deleted record should return DeletedSlot error"
    );
}

#[test]
fn heap_access_method_can_read_records_after_reopen() {
    let test_file = TestFile::new("heap_access_method_can_read_records_after_reopen");
    let page_size = 64;
    let capacity = 2;

    let first_record = vec![1_u8; 20];
    let second_record = vec![2_u8; 20];

    let (first_record_id, second_record_id, page_ids) = {
        let mut access_method = create_access_method(test_file.path(), page_size, capacity, vec![]);

        let first_record_id = access_method
            .insert(&first_record)
            .expect("failed to insert first record");
        let second_record_id = access_method
            .insert(&second_record)
            .expect("failed to insert second record");

        assert_ne!(first_record_id.page_id(), second_record_id.page_id());

        (
            first_record_id,
            second_record_id,
            access_method.page_ids().to_vec(),
        )
    };

    let access_method = create_access_method(test_file.path(), page_size, capacity, page_ids);

    let first = access_method
        .read(first_record_id)
        .expect("failed to read first record after reopen");
    let second = access_method
        .read(second_record_id)
        .expect("failed to read second record after reopen");

    assert_eq!(first, first_record);
    assert_eq!(second, second_record);
}
