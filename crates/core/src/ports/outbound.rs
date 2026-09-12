//! Puertos outbound: capacidades que la infraestructura implementa.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::errors::DomainError;
use crate::domain::note::Note;
use crate::domain::typestate::Formatted;
use crate::domain::value::{
    AudioDurationSecs, AudioFilePath, FormattedOutputStyle, LanguageIso, MarkdownContent,
    NoteTitle, RawTranscript,
};
use crate::ports::inbound::RemoteAudioRef;

/// Abstracción sobre los datos de audio que el engine de transcripción consume.
#[derive(Debug, Clone)]
pub enum AudioData<'a> {
    Bytes(&'a [u8]),
    Path(&'a AudioFilePath),
}

/// Errores del engine de transcripción.
#[derive(Debug, thiserror::Error)]
pub enum TranscriptionError {
    #[error("whisper engine failed: {0}")]
    Engine(String),
    #[error("unsupported audio input for engine")]
    UnsupportedInput,
    #[error("empty transcript returned by engine")]
    EmptyTranscript,
    #[error("timeout after {0}s")]
    Timeout(u64),
    #[error("language {0} not supported by engine")]
    UnsupportedLanguage(String),
}

/// Estrategia de transcripción. Implementado por adaptadores local (whisper-rs /
/// binario) y remoto (API).
#[async_trait]
pub trait TranscriptionEngine: Send + Sync {
    async fn transcribe(
        &self,
        audio: &AudioData<'_>,
        language: &LanguageIso,
    ) -> Result<RawTranscript, TranscriptionError>;
}

/// Metadatos no-textuales entregados al formateador.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteMeta {
    pub date: DateTime<Utc>,
    pub duration: Option<AudioDurationSecs>,
    pub language: LanguageIso,
    pub source: String,
}

/// Estrategia de formateo a Markdown.
#[async_trait]
pub trait Formatter: Send + Sync {
    async fn format(
        &self,
        transcript: &RawTranscript,
        title: NoteTitle,
        style: FormattedOutputStyle,
        meta: NoteMeta,
    ) -> Result<MarkdownContent, DomainError>;
}

/// Referencia de salida tras persistir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredReference {
    /// Ruta en disco donde se escribió el `.md`.
    FsPath(std::path::PathBuf),
    /// La salida se entregó por un canal externo (p. ej. chat), sin fichero.
    Outbound,
}

/// Errores de almacenamiento.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("permission denied writing to {0}")]
    PermissionDenied(String),
}

/// Persistencia del Markdown final.
#[async_trait]
pub trait Storage: Send + Sync {
    async fn persist(
        &self,
        note: &Note<Formatted>,
        content: &MarkdownContent,
    ) -> Result<StoredReference, StorageError>;
}

/// Errores al resolver audio remoto.
#[derive(Debug, thiserror::Error)]
pub enum AudioFetchError {
    #[error("download failed: {0}")]
    Download(String),
    #[error("file too large: {0} bytes (limit {1} bytes)")]
    TooLarge(u64, u64),
    #[error("file not found")]
    NotFound,
}

/// Descarga de audio referenciado de forma remota (p. ej. Telegram).
#[async_trait]
pub trait AudioRepository: Send + Sync {
    async fn fetch(&self, r#ref: &RemoteAudioRef) -> Result<Vec<u8>, AudioFetchError>;
}
