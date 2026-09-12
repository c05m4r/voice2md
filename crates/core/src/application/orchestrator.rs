//! Orquestador central: ata engine, formatter, storage y repository de audio.

use async_trait::async_trait;

use crate::domain::errors::DomainError;
use crate::domain::note::{Note, NoteSource};
use crate::domain::typestate::Formatted;
use crate::domain::value::{
    AudioDurationSecs, AudioFilePath, FormattedOutputStyle, LanguageIso, NoteTitle,
};
use crate::ports::inbound::{AudioInput, ProcessAudioRequest, ProcessOutcome, RemoteAudioRef};
use crate::ports::outbound::{
    AudioData, AudioRepository, Formatter, NoteMeta, Storage, TranscriptionEngine,
};

/// Límites operativos del procesamiento.
#[derive(Debug, Clone)]
pub struct Limits {
    pub max_duration_secs: u32,
    pub max_size_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_duration_secs: 300,
            max_size_bytes: 25 * 1024 * 1024,
        }
    }
}

/// Constructor del orquestador (inyección manual de dependencias).
#[derive(Default)]
pub struct OrchestratorBuilder {
    engine: Option<Box<dyn TranscriptionEngine>>,
    formatter: Option<Box<dyn Formatter>>,
    storage: Option<Box<dyn Storage>>,
    audio_repo: Option<Box<dyn AudioRepository>>,
    limits: Limits,
}

impl OrchestratorBuilder {
    #[must_use]
    pub fn engine(mut self, engine: Box<dyn TranscriptionEngine>) -> Self {
        self.engine = Some(engine);
        self
    }

    #[must_use]
    pub fn formatter(mut self, formatter: Box<dyn Formatter>) -> Self {
        self.formatter = Some(formatter);
        self
    }

    #[must_use]
    pub fn storage(mut self, storage: Box<dyn Storage>) -> Self {
        self.storage = Some(storage);
        self
    }

    #[must_use]
    pub fn audio_repo(mut self, repo: Box<dyn AudioRepository>) -> Self {
        self.audio_repo = Some(repo);
        self
    }

    #[must_use]
    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Ensambla el orquestador; falla si falta una dependencia obligatoria.
    pub fn build(self) -> Result<NoteOrchestrator, DomainError> {
        let engine = self
            .engine
            .ok_or_else(|| DomainError::Infrastructure("missing transcription engine".into()))?;
        let formatter = self
            .formatter
            .or_else(|| Some(Box::new(crate::application::DefaultFormatter)))
            .expect("formatter has default");
        let storage = self
            .storage
            .ok_or_else(|| DomainError::Infrastructure("missing storage".into()))?;
        Ok(NoteOrchestrator {
            engine,
            formatter,
            storage,
            audio_repo: self.audio_repo,
            limits: self.limits,
        })
    }
}

/// Audio resuelto y en propiedad (para extender el lifetime del `AudioData`).
enum ResolvedAudio {
    Bytes(Vec<u8>),
    Path(AudioFilePath),
}

/// Implementación por defecto del caso de uso central.
pub struct NoteOrchestrator {
    engine: Box<dyn TranscriptionEngine>,
    formatter: Box<dyn Formatter>,
    storage: Box<dyn Storage>,
    audio_repo: Option<Box<dyn AudioRepository>>,
    limits: Limits,
}

impl NoteOrchestrator {
    async fn resolve_audio(
        &self,
        input: &AudioInput,
    ) -> Result<(ResolvedAudio, NoteSource), DomainError> {
        match input {
            AudioInput::Local(path) => {
                let source = NoteSource::Cli {
                    origin_path: path.clone(),
                };
                Ok((ResolvedAudio::Path(path.clone()), source))
            }
            AudioInput::Remote(r) => match r {
                RemoteAudioRef::Telegram {
                    file_id,
                    chat_id,
                    user_id,
                } => {
                    let Some(repo) = &self.audio_repo else {
                        return Err(DomainError::Infrastructure(
                            "audio repository not configured for remote input".into(),
                        ));
                    };
                    let bytes = repo.fetch(r).await?;
                    if bytes.len() as u64 > self.limits.max_size_bytes {
                        return Err(DomainError::AudioTooLarge(
                            bytes.len() as u64,
                            self.limits.max_size_bytes,
                        ));
                    }
                    let _ = file_id;
                    let source = NoteSource::Telegram {
                        chat_id: *chat_id,
                        user_id: *user_id,
                    };
                    Ok((ResolvedAudio::Bytes(bytes), source))
                }
            },
        }
    }
}

#[async_trait]
impl ProcessAudioRequest for NoteOrchestrator {
    async fn process(
        &self,
        input: AudioInput,
        language: LanguageIso,
        style: FormattedOutputStyle,
        duration_hint: Option<u32>,
    ) -> Result<ProcessOutcome, DomainError> {
        let (audio, source) = self.resolve_audio(&input).await?;

        if let Some(dur) = duration_hint {
            if dur > self.limits.max_duration_secs {
                return Err(DomainError::AudioTooLong(
                    dur,
                    self.limits.max_duration_secs,
                ));
            }
        }

        let audio_data = match &audio {
            ResolvedAudio::Bytes(b) => AudioData::Bytes(b.as_slice()),
            ResolvedAudio::Path(p) => AudioData::Path(p),
        };

        let transcript = self.engine.transcribe(&audio_data, &language).await?;

        let note = Note::new(NoteTitle::derive_or_fallback(&transcript), language, source);
        let note = note.transcribe(transcript);

        let meta = NoteMeta {
            date: note.created_at(),
            duration: duration_hint.map(AudioDurationSecs::new).transpose()?,
            language: note.language().clone(),
            source: note.source().label().to_owned(),
        };

        let note: Note<Formatted> = note.format(self.formatter.as_ref(), style, meta).await?;
        let markdown = note.markdown().clone();

        self.storage.persist(&note, &markdown).await?;

        Ok(ProcessOutcome { note, markdown })
    }
}
