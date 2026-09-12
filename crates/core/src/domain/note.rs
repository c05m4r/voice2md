//! Entidad raíz `Note` con marcador de estado (typestate).

use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::errors::DomainError;
use crate::domain::typestate::{Formatted, New, Transcribed};
use crate::domain::value::{
    AudioFilePath, FormattedOutputStyle, LanguageIso, MarkdownContent, NoteTitle, RawTranscript,
};
use crate::ports::outbound::{Formatter, NoteMeta};

/// Identificador único de nota (UUIDv4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoteId(Uuid);

impl NoteId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for NoteId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NoteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// De dónde provino la nota.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NoteSource {
    Telegram { chat_id: i64, user_id: i64 },
    Cli { origin_path: AudioFilePath },
    File { origin_path: AudioFilePath },
}

impl NoteSource {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Telegram { .. } => "telegram",
            Self::Cli { .. } | Self::File { .. } => "file",
        }
    }
}

/// Entidad raíz del dominio. `State` indica la fase del pipeline.
#[derive(Debug, Clone)]
pub struct Note<State> {
    pub id: NoteId,
    pub title: NoteTitle,
    pub language: LanguageIso,
    pub created_at: DateTime<Utc>,
    pub source: NoteSource,
    state: State,
    _marker: PhantomData<fn() -> State>,
}

impl<State> Note<State> {
    #[must_use]
    pub fn id(&self) -> NoteId {
        self.id
    }

    #[must_use]
    pub fn title(&self) -> &NoteTitle {
        &self.title
    }

    #[must_use]
    pub fn language(&self) -> &LanguageIso {
        &self.language
    }

    #[must_use]
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    #[must_use]
    pub fn source(&self) -> &NoteSource {
        &self.source
    }
}

impl Note<New> {
    /// Crea una nota en estado inicial.
    #[must_use]
    pub fn new(title: NoteTitle, language: LanguageIso, source: NoteSource) -> Self {
        Self {
            id: NoteId::new(),
            title,
            language,
            created_at: Utc::now(),
            source,
            state: New,
            _marker: PhantomData,
        }
    }

    /// Da por asumido el transcript y avanza a `Transcribed`.
    #[must_use]
    pub fn transcribe(self, transcript: RawTranscript) -> Note<Transcribed> {
        Note {
            id: self.id,
            title: self.title,
            language: self.language,
            created_at: self.created_at,
            source: self.source,
            state: Transcribed { transcript },
            _marker: PhantomData,
        }
    }
}

impl Note<Transcribed> {
    #[must_use]
    pub fn transcript(&self) -> &RawTranscript {
        &self.state.transcript
    }

    /// Aplica el formateador y avanza a `Formatted`.
    pub async fn format(
        self,
        formatter: &dyn Formatter,
        style: FormattedOutputStyle,
        meta: NoteMeta,
    ) -> Result<Note<Formatted>, DomainError> {
        let markdown = formatter
            .format(&self.state.transcript, self.title.clone(), style, meta)
            .await?;
        Ok(Note {
            id: self.id,
            title: self.title,
            language: self.language,
            created_at: self.created_at,
            source: self.source,
            state: Formatted { markdown },
            _marker: PhantomData,
        })
    }
}

impl Note<Formatted> {
    #[must_use]
    pub fn markdown(&self) -> &MarkdownContent {
        &self.state.markdown
    }

    #[must_use]
    pub fn into_markdown(self) -> MarkdownContent {
        self.state.markdown
    }
}

impl NoteTitle {
    /// Deriva un título a partir de las primeras palabras del transcript.
    pub fn derive_from(transcript: &RawTranscript) -> Result<Self, DomainError> {
        let words: Vec<&str> = transcript.as_str().split_whitespace().take(7).collect();
        if words.is_empty() {
            return Err(DomainError::Untitled);
        }
        Self::new(words.join(" "))
    }

    /// Deriva un título con fallback a fecha + marca de tiempo.
    pub fn derive_or_fallback(transcript: &RawTranscript) -> Self {
        Self::derive_from(transcript).unwrap_or_else(|_| {
            let now = Utc::now().format("%Y-%m-%dT%H%M%S");
            // `now` no contiene caracteres inválidos, el título es siempre válido.
            Self::new(format!("nota-{now}")).expect("fallback title is always valid")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::DefaultFormatter;
    use crate::domain::value::{FormattedOutputStyle, RawTranscript};
    use crate::ports::outbound::NoteMeta;

    #[test]
    fn typestate_chain_produces_markdown() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let transcript = RawTranscript::new("una nota de voz de prueba").unwrap();
            let title = NoteTitle::derive_from(&transcript).unwrap();

            let note_new = Note::new(
                title,
                LanguageIso::new("es").unwrap(),
                NoteSource::Cli {
                    origin_path: place_holder_path(),
                },
            );

            let note_transcribed = note_new.transcribe(transcript);

            let meta = NoteMeta {
                date: Utc::now(),
                duration: None,
                language: LanguageIso::new("es").unwrap(),
                source: "test".to_owned(),
            };

            let note_formatted = note_transcribed
                .format(&DefaultFormatter, FormattedOutputStyle::Plain, meta)
                .await
                .unwrap();

            assert!(note_formatted.markdown().as_str().starts_with("# "));
        });
    }

    #[test]
    fn audio_path_rejects_non_audio_extension() {
        let p = std::env::temp_dir().join("voice2md-not-audio.txt");
        if !p.exists() {
            std::fs::write(&p, b"x").unwrap();
        }
        let err = AudioFilePath::try_from_path(p).unwrap_err();
        assert!(matches!(
            err,
            crate::domain::value::AudioFileError::UnsupportedExtension(_)
        ));
    }

    fn place_holder_path() -> AudioFilePath {
        let dir = std::env::temp_dir();
        let p = dir.join("voice2md-test-note.wav");
        if !p.exists() {
            std::fs::write(&p, b"placeholder").unwrap();
        }
        AudioFilePath::try_from_path(p).unwrap()
    }
}
