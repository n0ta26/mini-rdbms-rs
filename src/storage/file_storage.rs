use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use std::os::unix::fs::FileExt;

use crate::storage::engine::StorageEngine;
use crate::storage::error::{StorageError, StorageResult};

pub struct FileStorage {
    file: File,
    path: PathBuf,
}

impl FileStorage {
    pub fn open<P: AsRef<Path>>(path: P) -> StorageResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        Ok(FileStorage {
            file,
            path: path.as_ref().to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read_exact_loop(&self, offset: u64, buf: &mut [u8]) -> StorageResult<()> {
        let mut total_read = 0;

        while total_read < buf.len() {
            let current_offset = offset + total_read as u64;
            let target = &mut buf[total_read..];

            let read_size = self.file.read_at(target, current_offset)?;

            if read_size == 0 {
                return Err(StorageError::Unexpected {
                    offset,
                    expected: buf.len(),
                    actual: total_read,
                });
            }
            total_read += read_size;
        }
        Ok(())
    }

    fn write_exact_loop(&self, offset: u64, buf: &[u8]) -> StorageResult<()> {
        let mut total_written = 0;

        while total_written < buf.len() {
            let current_offset = offset + total_written as u64;
            let target = &buf[total_written..];

            let written_size = self.file.write_at(target, current_offset)?;

            if written_size == 0 {
                return Err(StorageError::Unexpected {
                    offset: current_offset,
                    expected: target.len(),
                    actual: total_written,
                });
            }
            total_written += written_size;
        }
        Ok(())
    }
}

impl StorageEngine for FileStorage {
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> StorageResult<()> {
        self.read_exact_loop(offset, buf)
    }

    fn write_exact_at(&self, offset: u64, buf: &[u8]) -> StorageResult<()> {
        self.write_exact_loop(offset, buf)
    }

    fn flush(&self) -> StorageResult<()> {
        let mut file = &self.file;
        file.flush()?;
        Ok(())
    }

    fn sync(&self) -> StorageResult<()> {
        self.file.sync_all()?;
        Ok(())
    }

    fn len(&self) -> StorageResult<u64> {
        let metadata = self.file.metadata()?;
        Ok(metadata.len())
    }

    fn truncate(&self, size: u64) -> StorageResult<()> {
        self.file.set_len(size)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::error::StorageError;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]

    fn open_creates_file_if_not_exists() {
        let test_file = TestFile::new("mini_rdbms_open_creates_file_if_not_exists.db");
        let path = test_file.path();

        let storage = FileStorage::open(path);

        assert!(storage.is_ok());
        assert!(path.exists());
    }

    #[test]
    fn new_file_is_empty() {
        let test_file = TestFile::new("mini_rdbms_new_file_is_empty.db");
        let path = test_file.path();

        let storage = FileStorage::open(path).expect("failed to open file storage");

        let is_empty = storage
            .is_empty()
            .expect("failed to check whether storage is empty");

        assert!(is_empty);

        assert_eq!(storage.len().expect("failed to get file length"), 0);
    }

    #[test]

    fn write_all_at_writes_data_at_offset() {
        let test_file = TestFile::new("mini_rdbms_write_all_at_writes_data_at_offset.db");
        let path = test_file.path();

        let storage = FileStorage::open(path).expect("failed to open file storage");

        storage
            .write_exact_at(0, b"hello")
            .expect("failed to write data");

        let mut buf = [0_u8; 5];

        storage
            .read_exact_at(0, &mut buf)
            .expect("failed to read data");

        assert_eq!(&buf, b"hello");
    }

    #[test]

    fn write_all_at_can_write_data_to_non_zero_offset() {
        let test_file =
            TestFile::new("mini_rdbms_write_all_at_can_write_data_to_non_zero_offset.db");
        let path = test_file.path();

        let storage = FileStorage::open(path).expect("failed to open file storage");

        storage
            .write_exact_at(100, b"page")
            .expect("failed to write data");

        let mut buf = [0_u8; 4];

        storage
            .read_exact_at(100, &mut buf)
            .expect("failed to read data");

        assert_eq!(&buf, b"page");
    }

    #[test]

    fn write_all_at_extends_file_size() {
        let test_file = TestFile::new("mini_rdbms_write_all_at_extends_file_size.db");
        let path = test_file.path();

        let storage = FileStorage::open(path).expect("failed to open file storage");

        storage
            .write_exact_at(100, b"hello")
            .expect("failed to write data");

        let len = storage.len().expect("failed to get file length");

        assert_eq!(len, 105);
    }

    #[test]

    fn read_exact_at_reads_exact_size() {
        let test_file = TestFile::new("mini_rdbms_read_exact_at_reads_exact_size.db");
        let path = test_file.path();

        let storage = FileStorage::open(path).expect("failed to open file storage");

        storage
            .write_exact_at(0, b"abcdef")
            .expect("failed to write data");

        let mut buf = [0_u8; 3];

        storage
            .read_exact_at(2, &mut buf)
            .expect("failed to read data");

        assert_eq!(&buf, b"cde");
    }

    #[test]

    fn read_exact_at_returns_unexpected_eof_when_short_read() {
        let test_file = TestFile::new("mini_rdbms_read_exact_at_returns_unexpected_eof.db");
        let path = test_file.path();

        let storage = FileStorage::open(path).expect("failed to open file storage");

        storage
            .write_exact_at(0, b"abc")
            .expect("failed to write data");

        let mut buf = [0_u8; 10];

        let result = storage.read_exact_at(0, &mut buf);

        println!("Result: {:?}", result);
        assert!(matches!(
            result,
            Err(StorageError::Unexpected {
                offset: 0,
                expected: 10,
                actual: 3,
            })
        ));
    }

    #[test]

    fn read_exact_at_returns_unexpected_eof_when_offset_is_beyond_file_size() {
        let test_file =
            TestFile::new("mini_rdbms_read_exact_at_returns_eof_when_offset_is_beyond_size.db");
        let path = test_file.path();

        let storage = FileStorage::open(path).expect("failed to open file storage");

        storage
            .write_exact_at(0, b"abc")
            .expect("failed to write data");

        let mut buf = [0_u8; 1];

        let result = storage.read_exact_at(100, &mut buf);

        assert!(matches!(
            result,
            Err(StorageError::Unexpected {
                offset: 100,

                expected: 1,

                actual: 0,
            })
        ));
    }

    #[test]

    fn truncate_can_shrink_file() {
        let test_file = TestFile::new("mini_rdbms_truncate_can_shrink_file.db");
        let path = test_file.path();

        let storage = FileStorage::open(path).expect("failed to open file storage");

        storage
            .write_exact_at(0, b"abcdef")
            .expect("failed to write data");

        storage.truncate(3).expect("failed to truncate file");

        assert_eq!(storage.len().expect("failed to get file length"), 3);

        let mut buf = [0_u8; 3];

        storage
            .read_exact_at(0, &mut buf)
            .expect("failed to read data");

        assert_eq!(&buf, b"abc");
    }

    #[test]

    fn truncate_can_extend_file() {
        let test_file = TestFile::new("mini_rdbms_truncate_can_extend_file.db");
        let path = test_file.path();

        let storage = FileStorage::open(path).expect("failed to open file storage");

        storage.truncate(4096).expect("failed to extend file");

        assert_eq!(storage.len().expect("failed to get file length"), 4096);
    }

    #[test]

    fn sync_all_succeeds_after_write() {
        let test_file = TestFile::new("mini_rdbms_sync_all_succeeds_after_write.db");
        let path = test_file.path();

        let storage = FileStorage::open(path).expect("failed to open file storage");

        storage
            .write_exact_at(0, b"durable")
            .expect("failed to write data");

        let result = storage.sync();

        assert!(result.is_ok());
    }

    #[test]

    fn data_can_be_read_after_reopening_file() {
        let test_file = TestFile::new("mini_rdbms_data_can_be_read_after_reopening_file.db");
        let path = test_file.path();

        {
            let storage = FileStorage::open(path).expect("failed to open file storage");

            storage
                .write_exact_at(0, b"persisted")
                .expect("failed to write data");

            storage.sync().expect("failed to sync file");
        }

        let reopened_storage = FileStorage::open(path).expect("failed to reopen file storage");

        let mut buf = [0_u8; 9];

        reopened_storage
            .read_exact_at(0, &mut buf)
            .expect("failed to read data after reopening");

        assert_eq!(&buf, b"persisted");
    }
}
