//! Errores del dominio (`thiserror`). Los binarios usan `anyhow`, nunca este tipo
//! se expone en crates de infraestructura.

use crate::ports::outbound::{AudioFetchError, StorageError, TranscriptionError};

/// Error raíz del dominio/aplicación.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("empty transcript returned by engine")]
    EmptyTranscript,

    #[error("empty markdown produced by formatter")]
    EmptyMarkdown,

    #[error("invalid title: {0:?}")]
    InvalidTitle(String),

    #[error("invalid duration: must be > 0")]
    InvalidDuration,

    #[error("invalid or unsupported language")]
    InvalidLanguage,

    #[error("invalid output style: {0}")]
    InvalidStyle(String),

    #[error("cannot derive a title from an empty transcript")]
    Untitled,

    #[error("audio exceeds maximum duration: {0}s > {1}s")]
    AudioTooLong(u32, u32),

    #[error("audio exceeds maximum size: {0} > {1} bytes")]
    AudioTooLarge(u64, u64),

    #[error("transcription error: {0}")]
    Transcription(#[from] TranscriptionError),

    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("audio fetch error: {0}")]
    AudioFetch(#[from] AudioFetchError),

    #[error("infrastructure error: {0}")]
    Infrastructure(String),
}
