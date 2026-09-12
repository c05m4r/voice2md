//! Puertos inbound: casos de uso expuestos al exterior.

use async_trait::async_trait;

use crate::domain::errors::DomainError;
use crate::domain::note::Note;
use crate::domain::typestate::Formatted;
use crate::domain::value::{AudioFilePath, FormattedOutputStyle, LanguageIso, MarkdownContent};

/// Referencia a un audio que no está en disco local. El adaptador de entrada lo
/// resuelve (p. ej. descarga desde Telegram) vía `AudioRepository`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RemoteAudioRef {
    /// Fichero de voz de Telegram, con contexto del chat y usuario.
    Telegram {
        file_id: String,
        chat_id: i64,
        user_id: i64,
    },
}

/// Origen de audio entrante.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AudioInput {
    /// Ruta local ya validada.
    Local(AudioFilePath),
    /// Referencia remota a resolver antes de transcribir.
    Remote(RemoteAudioRef),
}

/// Resultado del pipeline completo.
#[derive(Debug, Clone)]
pub struct ProcessOutcome {
    pub note: Note<Formatted>,
    pub markdown: MarkdownContent,
}

/// Caso de uso central: ingest -> transcribe -> format -> persist.
#[async_trait]
pub trait ProcessAudioRequest: Send + Sync {
    async fn process(
        &self,
        input: AudioInput,
        language: LanguageIso,
        style: FormattedOutputStyle,
        duration_hint: Option<u32>,
    ) -> Result<ProcessOutcome, DomainError>;
}
