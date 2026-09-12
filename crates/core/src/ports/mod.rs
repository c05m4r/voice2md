//! Puertos: traits que definen los bordes de la aplicación (hexagonal).
//!
//! - `inbound`: casos de uso que el exterior invoca.
//! - `outbound`: capacidades que el dominio requiere de la infraestructura.

pub mod inbound;
pub mod outbound;

pub use inbound::{AudioInput, ProcessAudioRequest, ProcessOutcome, RemoteAudioRef};
pub use outbound::{
    AudioData, AudioFetchError, AudioRepository, Formatter, NoteMeta, Storage, StorageError,
    StoredReference, TranscriptionEngine, TranscriptionError,
};
