//! Entidades, value objects, enums y reglas de negocio.

pub mod errors;
pub mod note;
pub mod typestate;
pub mod value;

pub use errors::DomainError;
pub use note::{Note, NoteId, NoteSource};
pub use typestate::{Failed, Formatted, New, Transcribed};
pub use value::{
    AudioDurationSecs, AudioFilePath, AudioFormat, FormattedOutputStyle, LanguageIso,
    MarkdownContent, NoteTitle, RawTranscript,
};
