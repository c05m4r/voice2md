//! `Storage` que escribe el Markdown a disco.

use std::path::PathBuf;

use async_trait::async_trait;
use tracing::debug;

use voice2md_core::{
    MarkdownContent, Note, Storage, StorageError, StoredReference, typestate::Formatted,
};

/// Escribe `<out_dir>/<título>.md`.
#[derive(Debug, Clone)]
pub struct FileStorage {
    out_dir: PathBuf,
}

impl FileStorage {
    #[must_use]
    pub fn new(out_dir: PathBuf) -> Self {
        Self { out_dir }
    }
}

#[async_trait]
impl Storage for FileStorage {
    async fn persist(
        &self,
        note: &Note<Formatted>,
        content: &MarkdownContent,
    ) -> Result<StoredReference, StorageError> {
        tokio::fs::create_dir_all(&self.out_dir).await?;
        let mut name = note.title().as_str().to_owned();
        name.push_str(".md");
        let path = self.out_dir.join(name);
        debug!(path = %path.display(), "writing markdown");
        tokio::fs::write(&path, content.as_str()).await?;
        Ok(StoredReference::FsPath(path))
    }
}
