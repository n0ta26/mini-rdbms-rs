use std::fs::{File, OpenOptions};
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
            .open(&path)?;

        Ok(FileStorage {
            file,
            path: path.as_ref().to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read_exact_loop(&self, mut offset: u64, mut buf: &mut [u8]) -> StorageResult<()> {
        let mut total_read = 0;

        while total_read < buf.len() {
            let current_offset = offset + total_read as u64;
            let target = &mut buf[total_read..];

            let read_size = self.file.read_at(target, current_offset)?;

            if read_size == 0 {
                return Err(StorageError::Unexpected {
                    offset: current_offset,
                    expected: target.len(),
                    actual: total_read,
                });
            }
            total_read += read_size;
        }
        Ok(())
    }

    fn write_exact_loop(&self, mut offset: u64, mut buf: &[u8]) -> StorageResult<()> {
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
        self.file.flush()?;
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
