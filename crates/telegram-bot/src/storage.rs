//! `Storage` que escribe el Markdown a disco: `<out_dir>/<carpeta_usuario>/<título_snake_case>_<timestamp>.md`.

use std::path::PathBuf;

use async_trait::async_trait;
use chrono::Utc;
use tracing::{debug, warn};

use voice2md_core::{
    MarkdownContent, Note, NoteSource, StoredReference, Storage, StorageError, typestate::Formatted,
};

/// Escribe el `.md` bajo `<out_dir>/<user_dir>/<slug>_<timestamp>.md`.
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
        let Some(slug) = snake_case(note.title().as_str()) else {
            warn!(title = note.title().as_str(), "title yields empty slug");
            return Ok(StoredReference::Outbound);
        };

        let user_dir = user_dir_of(note.source());
        let timestamp = note.created_at().with_timezone(&Utc);
        let name = format!(
            "{slug}_{}",
            timestamp.format("%Y%m%dT%H%M%SZ")
        );

        let dir = self.out_dir.join(user_dir);
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{name}.md"));
        debug!(path = %path.display(), "writing markdown");
        tokio::fs::write(&path, content.as_str()).await?;
        Ok(StoredReference::FsPath(path))
    }
}

/// Carpeta por usuario. Telegram usa el `user_id`; CLI/archivo usan `local`.
fn user_dir_of(source: &NoteSource) -> String {
    match source {
        NoteSource::Telegram { user_id, .. } => format!("user_{user_id}"),
        NoteSource::Cli { .. } | NoteSource::File { .. } => "local".to_owned(),
    }
}

/// Convierte el título a `snake_case` (minúsculas, separadores a `_`,
/// sin caracteres de control). Devuelve `None` si queda vacío.
fn snake_case(title: &str) -> Option<String> {
    let mut out = String::with_capacity(title.len());
    let mut prev_sep = true;

    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
            prev_sep = false;
        } else if !prev_sep {
            out.push('_');
            prev_sep = true;
        }
    }

    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
