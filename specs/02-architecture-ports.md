# 02 — Arquitectura: Puertos & Adaptadores (Hexagonal)

> Define los **Trait** que actúan como puertos. Los adaptadores (Telegram, whisper.rs,
> filesystem, API remota) implementan estos traits y **nunca** entran al dominio.

## 1. Regla de dependencia

```
        ┌────────────────────────────┐
        │   Domain (core)            │
        │   Nota, Value Objects,     │
        │   Typestate, reglas        │
        └──────────────┬─────────────┘
                       │ dependencias apuntan hacia DENTRO
        ┌──────────────▼─────────────┐
        │   Ports (traits)           │
        │   Inbound: CliUseCases     │
        │   Outbound: Transcription  │
        │            Engine, Storage │
        └──────────────┬─────────────┘
                       │ implementan
     ┌─────────────────┼─────────────────────┐
     ▼                 ▼                      ▼
┌──────────┐    ┌──────────────┐    ┌──────────────┐
│ Telegram │    │ whisper.rs / │    │  filesystem  │
│ adapter  │    │ Remote API   │    │  / stdout    │
└──────────┘    └──────────────┘    └──────────────┘
```

- El crate `core` contiene el dominio + las **definiciones de los traits** (puertos).
- Nada en `core` importa crates de infraestructura.
- Los adaptadores dependen de `core` y de su SDK, nunca al revés.

## 2. Puertos Inbound

Los puertos inbound son los **casos de uso** que el exterior puede invocar. Los
implementa el dominio (componentes de aplicación) y los consumen los adaptadores.

### 2.1 `ProcessNote` (caso de uso central)

```rust
#[async_trait]
pub trait ProcessAudioRequest {
    /// Orquesta todo el pipeline: ingest -> transcribe -> format -> persist.
    /// Devuelve el Markdown final y la nota formateada.
    async fn process(
        &self,
        input: AudioInput,
        language: LanguageIso,
        style: FormattedOutputStyle,
    ) -> Result<ProcessOutcome, DomainError>;
}

pub struct ProcessOutcome {
    pub note: Note<Formatted>,
    pub markdown: MarkdownContent,
}
```

### 2.2 `AudioInput` (unión de orígenes entrantes)

```rust
pub enum AudioInput {
    /// Path local ya validado (CLI / file adapter).
    Local(AudioFilePath),
    /// Referencia remota que el adaptador de entrada debe resolver a bytes/local.
    Remote(RemoteAudioRef),
}
```

`RemoteAudioRef` es un enum genérico que los adaptadores llenan:

```rust
pub enum RemoteAudioRef {
    Telegram(TelegramFileId),
    // extensible: S3(ObjectKey), HTTP(Url), etc.
}
```

### 2.3 Contrato del Inbound Adapter

Todo adaptador inbound (CLI, bot) debe:

1. Resolver `AudioInput` (validando/descargando el audio).
2. Invocar `ProcessAudioRequest::process` con `language` y `style` configurados.
3. Entregar el `ProcessOutcome` a su canal de salida (mensaje Telegram / archivo).

## 3. Puertos Outbound

Los implementan los adaptadores (infraestructura). Define firmas exactas.

### 3.1 `TranscriptionEngine` (Strategy / Trait Object)

```rust
#[async_trait]
pub trait TranscriptionEngine: Send + Sync {
    /// Transcribe audio (bytes o path) a un transcript crudo.
    /// El engine decide internamente si usa whisper local o API remota.
    async fn transcribe(
        &self,
        audio: &AudioData,
        language: &LanguageIso,
    ) -> Result<RawTranscript, TranscriptionError>;
}

/// Abstracción sobre los datos de audio que el engine necesita.
pub enum AudioData<'a> {
    Bytes(&'a [u8]),
    Path(&'a AudioFilePath),
}

#[derive(Debug, thiserror::Error)]
pub enum TranscriptionError {
    #[error("whisper process failed: {0}")]
    Engine(String),
    #[error("unsupported audio data for engine")]
    UnsupportedInput,
    #[error("empty transcript returned")]
    EmptyTranscript,
    #[error("timeout after {0}s")]
    Timeout(u64),
    #[error("language {0} not supported")]
    UnsupportedLanguage(String),
}
```

**Decisión de diseño — intercambiabilidad:** al ser `dyn TranscriptionEngine`, el
componente de aplicación recibe `&dyn TranscriptionEngine` y no sabe si es:

- `LocalWhisperEngine` (`whisper.rs`, C bindings)
- `RemoteWhisperEngine` (API REST, p. ej. OpenAI)

### 3.2 `Formatter`

```rust
#[async_trait]
pub trait Formatter: Send + Sync {
    /// Convierte transcript crudo en Markdown estructurado según estilo.
    async fn format(
        &self,
        transcript: &RawTranscript,
        title: NoteTitle,
        style: FormattedOutputStyle,
        meta: NoteMeta,
    ) -> Result<MarkdownContent, DomainError>;
}
```

`NoteMeta` agrupa metadatos no textuales (fecha, duración, idioma, origen) para
el frontmatter.

### 3.3 `Storage`

```rust
#[async_trait]
pub trait Storage: Send + Sync {
    /// Persiste la nota Markdown y devuelve una referencia/locación de salida.
    async fn persist(
        &self,
        note: &Note<Formatted>,
        content: &MarkdownContent,
    ) -> Result<StoredReference, StorageError>;
}

pub enum StoredReference {
    FsPath(AudioFilePath),      // para CLI/file
    Outbound(OutboundRef),      // para bot: lo que se manda por chat
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("permission denied writing to {0}")]
    PermissionDenied(String),
}
```

> En la práctica para el bot, persistir puede ser un no-op y la entrega real es
> enviar el texto por chat; `StoredReference::Outbound` cubre ese caso.

### 3.4 `AudioRepository` (descarga/resolución de audio remoto)

```rust
#[async_trait]
pub trait AudioRepository: Send + Sync {
    /// Resuelve una referencia remota (p.ej. TelegramFileId) a bytes de audio.
    async fn fetch(&self, r#ref: &RemoteAudioRef) -> Result<Vec<u8>, AudioFetchError>;
}

#[derive(Debug, thiserror::Error)]
pub enum AudioFetchError {
    #[error("download failed: {0}")]
    Download(String),
    #[error("file too large: {0} bytes (limit {1} bytes)")]
    TooLarge(u64, u64),
    #[error("file not found")]
    NotFound,
}
```

## 4. Orquestador (Application Service)

Vive en `core`, depende **solo de los traits**:

```rust
pub struct NoteOrchestrator {
    engine: Box<dyn TranscriptionEngine>,
    formatter: Box<dyn Formatter>,
    storage: Box<dyn Storage>,
    audio_repo: Option<Box<dyn AudioRepository>>,
}

impl NoteOrchestrator {
    pub fn builder() -> OrchestratorBuilder { /* ... */ }
}

#[async_trait]
impl ProcessAudioRequest for NoteOrchestrator {
    async fn process(&self, input: AudioInput, language: LanguageIso, style: FormattedOutputStyle)
        -> Result<ProcessOutcome, DomainError>
    {
        let audio = self.resolve_audio(input).await?;       // Remote -> bytes via audio_repo
        let transcript = self.engine.transcribe(&audio, &language).await?;
        let note = Note::new(title_from(&transcript), language, source)?;
        let note = note.transcribe(&*self.engine)?;          // Nota: reusa transcript
        let note = note.format(&*self.formatter)?;
        let md = note.markdown().clone();
        self.storage.persist(&note, &md).await?;
        Ok(ProcessOutcome { note, markdown: md })
    }
}
```

> Nota: esta es la **firma** de referencia para Fase 1; el detalle de si
> `transcribe` recibe el audio ya resuelto por el orquestador o directamente el
> engine se fija en la fase de implementación. Lo relevante ahora es la separación.

## 5. Inyección de dependencias

Se prefiere **constructor injection** manual (sin framework):

- `OrchestratorBuilder` en `core` ensambla implementaciones concretas.
- Cada binario (CLI, bot) construye su `NoteOrchestrator` con los adaptadores que
  le corresponden.
- Para tests: implementaciones `MockTranscriptionEngine`, `InMemoryStorage`, etc.
  en un crate `core` o `test-support`.

## 6. Mapas de errores (boundary)

| Capa | Tipo de error | Crate |
|------|--------------|-------|
| Dominio | `DomainError` (thiserror) | `core` |
| Aplicación | `DomainError` envuelve infra | `core` |
| Infra trascripción | `TranscriptionError` | `transcriber` |
| Infra storage | `StorageError` | `core` (definido como puerto) |
| Infra audio | `AudioFetchError` | `core` (definido como puerto) |
| Binarios | `anyhow::Error` | `cli`, `telegram_bot` |

Los errores de infraestructura se convierten a `DomainError::Infrastructure` en el
orquestador, de modo que el dominio nunca expone errores de `reqwest`/`whisper`.

## 7. Concurrencia (contrato)

El processing pesado (transcripción) **no** debe bloquear al bot. Contrato:

- El bot encola las peticiones en `tokio::sync::mpsc::Sender<Job>`.
- Un pool de workers (N = `available_parallelism`) consume la cola y ejecuta
  `process` en `tokio::task::spawn`.
- `TranscriptionEngine::transcribe` es `async` y debe ser `Send` (puede correr en
  `spawn_blocking` si usa bindings C síncronos de whisper).

```rust
pub struct Job {
    pub input: AudioInput,
    pub language: LanguageIso,
    pub style: FormattedOutputStyle,
    pub reply: oneshot::Sender<Result<ProcessOutcome, DomainError>>,
}
```

---

*Documento vivo. Todo trait nuevo debe registrarse aquí y mantener la regla de
dependencia hexagonal.*
