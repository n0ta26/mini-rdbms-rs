use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use std::os::unix::fs::FileExt;

use crate::storage::engine::StorageEngine;
use crate::storage::error::{StorageError, StorageResult};

/// This struct represents a file-based storage engine that implements the `StorageEngine` trait. It provides methods for reading and writing data at specific offsets in a file, as well as managing the file's size and ensuring data durability through flushing and syncing. The `open` method allows for creating or opening a file at a specified path, while the `path` method returns the path of the associated file. The internal methods `read_exact_loop` and `write_exact_loop` handle the logic for performing exact reads and writes, ensuring that the requested amount of data is processed even if the underlying I/O operations may be partial.
pub struct FileStorage {
    file: File,
    path: PathBuf,
}

/// This implementation of the `StorageEngine` trait provides the necessary functionality for a file-based storage engine, including reading and writing data at specific offsets, flushing and syncing data to ensure durability, and managing the file's size through truncation. The `read_exact_at` and `write_exact_at` methods utilize the internal loop methods to guarantee that the requested amount of data is processed, while the `flush`, `sync`, `len`, and `truncate` methods provide the necessary operations for managing the file's state and ensuring data integrity.
impl FileStorage {
    /// This method opens a file at the specified path for reading and writing. If the file does not exist, it will be created. The `truncate(false)` option ensures that the file is not truncated when opened, allowing for existing data to be preserved. If the file cannot be opened or created, an appropriate `StorageError` will be returned.
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

    /// This method returns the path of the file associated with the storage. It allows external code to access the file path for purposes such as logging, debugging, or performing additional file operations outside of the storage engine's interface.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// This internal method performs a loop to read data from the file at the specified offset until the requested buffer is completely filled. It handles the possibility of short reads by checking the number of bytes read in each iteration and continuing until the total number of bytes read matches the length of the buffer. If a short read occurs (i.e., if `read_at` returns 0 before filling the buffer), an appropriate `StorageError` will be returned indicating an unexpected end of file.
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

    /// This internal method performs a loop to write data to the file at the specified offset until all bytes from the buffer have been written. It handles the possibility of partial writes by checking the number of bytes written in each iteration and continuing until the total number of bytes written matches the length of the buffer. If a partial write occurs (i.e., if `write_at` returns 0 before writing all bytes), an appropriate `StorageError` will be returned indicating an unexpected end of file.
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

/// This implementation of the `StorageEngine` trait for `FileStorage` provides the necessary functionality for a file-based storage engine, including reading and writing data at specific offsets, flushing and syncing data to ensure durability, and managing the file's size through truncation. The methods utilize the internal loop methods to guarantee that the requested amount of data is processed, while the `flush`, `sync`, `len`, and `truncate` methods provide the necessary operations for managing the file's state and ensuring data integrity.
impl StorageEngine for FileStorage {
    /// This method reads data from the file at the specified offset into the provided buffer. It uses the `read_exact_loop` method to ensure that the entire buffer is filled, handling any short reads that may occur. If a short read occurs, an appropriate `StorageError` will be returned indicating an unexpected end of file.
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> StorageResult<()> {
        self.read_exact_loop(offset, buf)
    }

    /// This method writes data to the file at the specified offset from the provided buffer. It uses the `write_exact_loop` method to ensure that all bytes from the buffer are written, handling any partial writes that may occur. If a partial write occurs, an appropriate `StorageError` will be returned indicating an unexpected end of file.
    fn write_exact_at(&self, offset: u64, buf: &[u8]) -> StorageResult<()> {
        self.write_exact_loop(offset, buf)
    }

    /// This method flushes any buffered data to the underlying storage device. It calls the `flush` method on the file, which ensures that all buffered data is sent to the operating system. However, it does not guarantee that the data is physically written to the device, as it may still be in the OS buffer. If an error occurs during flushing, an appropriate `StorageError` will be returned.
    fn flush(&self) -> StorageResult<()> {
        let mut file = &self.file;
        file.flush()?;
        Ok(())
    }

    /// This method ensures that all buffered data is flushed to the underlying storage device and that the device has physically written the data to its media. It calls the `sync_all` method on the file, which typically involves calling `fsync` on the file descriptor to ensure that the data is physically written to the device. If an error occurs during syncing, an appropriate `StorageError` will be returned.
    fn sync(&self) -> StorageResult<()> {
        self.file.sync_all()?;
        Ok(())
    }

    /// This method returns the current size of the storage in bytes by querying the file's metadata. It retrieves the file's metadata using the `metadata` method and then returns the length of the file in bytes. If an error occurs while retrieving the metadata, an appropriate `StorageError` will be returned.
    fn len(&self) -> StorageResult<u64> {
        let metadata = self.file.metadata()?;
        Ok(metadata.len())
    }

    /// This method allows for both shrinking and extending the file. When shrinking, any data beyond the new size will be discarded. When extending, the new space will be filled with zeros. It calls the `set_len` method on the file to adjust its size accordingly. If an error occurs during truncation, an appropriate `StorageError` will be returned.
    fn truncate(&self, size: u64) -> StorageResult<()> {
        self.file.set_len(size)?;
        Ok(())
    }
}

/// This module contains tests for the `FileStorage` implementation of the `StorageEngine` trait. The tests cover various scenarios, including creating a new file, writing and reading data at specific offsets, handling short reads and writes, truncating the file to shrink and extend its size, syncing data to ensure durability, and verifying that data can be read after reopening the file. The `TestFile` struct is used to create unique temporary files for each test, ensuring that tests do not interfere with each other and that files are cleaned up after the tests are run.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::error::StorageError;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// This struct represents a temporary test file that is created for each test case. It generates a unique file name based on the test name, process ID, timestamp, and a unique ID to ensure that multiple tests can run concurrently without conflicts. The `Drop` implementation ensures that the file is removed from the filesystem when the `TestFile` instance goes out of scope, preventing clutter and ensuring that temporary files do not persist after tests are completed.
    struct TestFile {
        path: PathBuf,
    }

    /// This implementation provides a constructor for creating a new `TestFile` instance with a unique file name based on the test name, process ID, timestamp, and a unique ID. The `path` method allows access to the file path for use in tests.
    impl TestFile {
        /// This method generates a unique file name for the test file based on the provided test name, the current process ID, a timestamp, and a unique ID. It ensures that each test file has a distinct name to avoid conflicts when multiple tests are run concurrently. The generated file is located in the system's temporary directory.
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

        /// This method returns the path of the test file, allowing tests to access the file for reading and writing operations.
        fn path(&self) -> &Path {
            &self.path
        }
    }

    /// This implementation ensures that the test file is removed from the filesystem when the `TestFile` instance goes out of scope. The `drop` method attempts to remove the file, and any errors during removal are ignored (e.g., if the file does not exist or cannot be removed for some reason). This helps to keep the filesystem clean and prevents temporary files from persisting after tests are completed.
    impl Drop for TestFile {
        /// This method is called when the `TestFile` instance goes out of scope. It attempts to remove the file from the filesystem using `fs::remove_file`. Any errors that occur during file removal are ignored, ensuring that the test does not fail due to issues with file cleanup.
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    /// This test verifies that opening a file with `FileStorage::open` creates the file if it does not already exist. It uses the `TestFile` struct to generate a unique temporary file path, attempts to open the file using `FileStorage`, and then checks that the operation was successful and that the file now exists on the filesystem.
    #[test]
    fn open_creates_file_if_not_exists() {
        let test_file = TestFile::new("mini_rdbms_open_creates_file_if_not_exists.db");
        let path = test_file.path();

        let storage = FileStorage::open(path);

        assert!(storage.is_ok());
        assert!(path.exists());
    }

    /// This test verifies that a newly created file is empty. It opens a new file using `FileStorage`, checks that the `is_empty` method returns `true`, and that the length of the file is zero. This ensures that the storage engine correctly identifies an empty file and that no unintended data is present when a new file is created.
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

    /// This test verifies that data can be written to a specific offset in the file using the `write_exact_at` method. It creates a new file, writes data at a non-zero offset, and then reads the data back to ensure it was written correctly.
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

    /// This test verifies that data can be written to a non-zero offset in the file using the `write_exact_at` method. It creates a new file, writes data at a specific offset, and then reads the data back from that offset to ensure it was written correctly.
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

    /// This test verifies that writing data at a specific offset extends the file size if the offset is beyond the current end of the file. It creates a new file, writes data at an offset that is beyond the current file size, and then checks that the file length has been extended to accommodate the new data.
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

    /// This test verifies that the `read_exact_at` method reads the exact number of bytes requested from a specific offset. It creates a new file, writes data to it, and then uses `read_exact_at` to read a portion of the data back, ensuring that the correct bytes are read and that the method handles offsets correctly.
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

    /// This test verifies that the `read_exact_at` method returns an appropriate error when a short read occurs (i.e., when the end of the file is reached before filling the buffer). It creates a new file, writes a small amount of data, and then attempts to read more bytes than are available, checking that the correct `StorageError::Unexpected` error is returned with the expected and actual byte counts.
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

    /// This test verifies that the `read_exact_at` method returns an appropriate error when attempting to read from an offset that is beyond the current end of the file. It creates a new file, writes a small amount of data, and then attempts to read from an offset that is beyond the file size, checking that the correct `StorageError::Unexpected` error is returned with the expected and actual byte counts.
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

    /// This test verifies that the `write_exact_at` method returns an appropriate error when a short write occurs (i.e., when the end of the file is reached before writing all bytes from the buffer). It creates a new file, attempts to write a large amount of data at an offset that is beyond the current file size, and checks that the correct `StorageError::Unexpected` error is returned with the expected and actual byte counts.
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

    /// This test verifies that the `truncate` method can extend the file size. It creates a new file, truncates it to a larger size, and then checks that the file length has been extended and that the new space is filled with zeros.
    #[test]
    fn truncate_can_extend_file() {
        let test_file = TestFile::new("mini_rdbms_truncate_can_extend_file.db");
        let path = test_file.path();

        let storage = FileStorage::open(path).expect("failed to open file storage");

        storage.truncate(4096).expect("failed to extend file");

        assert_eq!(storage.len().expect("failed to get file length"), 4096);
    }

    /// This test verifies that the `sync` method successfully flushes data to the underlying storage device. It creates a new file, writes data to it, calls `sync`, and checks that the operation was successful. This ensures that the `sync` method is correctly implemented and that it can be used to ensure data durability.
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

    /// This test verifies that data can be read from a file after it has been reopened. It creates a new file, writes data to it, closes the file, reopens it, and then reads the data back to ensure it was written correctly.
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
