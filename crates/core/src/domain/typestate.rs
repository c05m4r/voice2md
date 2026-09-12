//! Estados del pipeline (typestate pattern).
//!
//! Cada estado es un tipo distinto. `Note<State>` guarda el payload del estado
//! actual como campo concreto (no `Option`), de modo que el compilador garantiza
//! la presencia del transcript y del markdown exactamente en las fases correctas.

use crate::domain::value::{MarkdownContent, RawTranscript};

/// Nota creada, audio disponible, sin transcribir. Sin payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct New;

/// Audio transcrito. Lleva el `RawTranscript`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Transcribed {
    pub(crate) transcript: RawTranscript,
}

/// Transcript ya formateado. Lleva el `MarkdownContent` final.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Formatted {
    pub(crate) markdown: MarkdownContent,
}

/// Estado terminal de error (retenido para trazabilidad).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Failed {
    pub(crate) error: String,
}
