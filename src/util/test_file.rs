use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TestFile {
    path: PathBuf,
}

impl TestFile {
    pub fn new(test_name: &str) -> Self {
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

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
