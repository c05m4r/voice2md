//! Núcleo del dominio: puertos, entidades y casos de uso.
//!
//! Este crate define todo lo que es lógica de negocio (dominio), los puertos
//! (traits) de entrada/salida y el orquestador que ata ambas partes. No depende
//! de infraestructura (SDK de Telegram, whisper, red).

pub mod application;
pub mod domain;
pub mod ports;

pub use application::{DefaultFormatter, Limits, NoteOrchestrator, OrchestratorBuilder};
pub use domain::errors::DomainError;
pub use domain::typestate;
pub use domain::{
    AudioDurationSecs, AudioFilePath, AudioFormat, FormattedOutputStyle, LanguageIso,
    MarkdownContent, Note, NoteId, NoteSource, NoteTitle, RawTranscript,
};
pub use ports::inbound::{AudioInput, ProcessAudioRequest, ProcessOutcome, RemoteAudioRef};
pub use ports::outbound::{
    AudioData, AudioFetchError, AudioRepository, Formatter, NoteMeta, Storage, StorageError,
    StoredReference, TranscriptionEngine, TranscriptionError,
};
