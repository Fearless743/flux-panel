//! recorder/file：JSONL file 持久化。

use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Result;

pub struct FileRecorder {
    path: PathBuf,
    inner: Mutex<Option<BufWriter<std::fs::File>>>,
}

impl FileRecorder {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            inner: Mutex::new(None),
        }
    }

    fn ensure_open(&self) -> Result<()> {
        let mut guard = self.inner.lock().unwrap();
        if guard.is_none() {
            let f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            *guard = Some(BufWriter::new(f));
        }
        Ok(())
    }

    /// 写一条 JSON 行事件。
    pub fn record<T: serde::Serialize>(&self, event: &T) -> Result<()> {
        self.ensure_open()?;
        let mut guard = self.inner.lock().unwrap();
        let w = guard.as_mut().unwrap();
        let s = serde_json::to_string(event)?;
        writeln!(w, "{}", s)?;
        w.flush()?;
        Ok(())
    }

    pub fn flush(&self) -> Result<()> {
        let mut guard = self.inner.lock().unwrap();
        if let Some(w) = guard.as_mut() {
            w.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_to_tmpfile() {
        let tmp = std::env::temp_dir().join(format!("gost-rs-test-{}.jsonl", std::process::id()));
        let r = FileRecorder::new(&tmp);
        #[derive(serde::Serialize)]
        struct E { a: i32 }
        r.record(&E { a: 1 }).unwrap();
        r.record(&E { a: 2 }).unwrap();
        let content = std::fs::read_to_string(&tmp).unwrap();
        assert!(content.contains("\"a\":1"));
        assert!(content.contains("\"a\":2"));
        let _ = std::fs::remove_file(&tmp);
    }
}
