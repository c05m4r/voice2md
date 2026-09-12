//! Motor Whisper remoto vía API REST (p. ej. OpenAI `/audio/transcriptions`).

use async_trait::async_trait;
use serde::Deserialize;
use tracing::debug;

use voice2md_core::{
    AudioData, LanguageIso, RawTranscript, TranscriptionEngine, TranscriptionError,
};

/// Configuración del motor remoto.
#[derive(Debug, Clone)]
pub struct RemoteWhisperConfig {
    /// URL del endpoint de transcripción.
    pub endpoint: String,
    /// Clave API (Authorization: Bearer). Si es `None` y `api_key_env` está
    /// definido, se lee de esa variable de entorno.
    pub api_key: Option<String>,
    /// Nombre de la variable de entorno con la clave.
    pub api_key_env: String,
    /// Modelo a solicitar (p. ej. `whisper-1`).
    pub model: String,
    /// Segundos antes de abortar la petición.
    pub timeout_secs: u64,
}

impl Default for RemoteWhisperConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://api.openai.com/v1/audio/transcriptions".to_owned(),
            api_key: None,
            api_key_env: "OPENAI_API_KEY".to_owned(),
            model: "whisper-1".to_owned(),
            timeout_secs: 120,
        }
    }
}

#[derive(Deserialize)]
struct TranscribeResponse {
    text: String,
}

/// Motor remoto. Envía los bytes de audio como `multipart/form-data`.
#[derive(Debug, Clone)]
pub struct RemoteWhisperEngine {
    config: RemoteWhisperConfig,
    client: reqwest::Client,
}

impl RemoteWhisperEngine {
    pub fn new(config: RemoteWhisperConfig) -> Result<Self, TranscriptionError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| TranscriptionError::Engine(format!("http client: {e}")))?;
        Ok(Self { config, client })
    }

    fn api_key(&self) -> Result<String, TranscriptionError> {
        if let Some(k) = &self.config.api_key {
            return Ok(k.clone());
        }
        std::env::var(&self.config.api_key_env).map_err(|_| {
            TranscriptionError::Engine(format!(
                "missing API key: set {} or provide api_key",
                self.config.api_key_env
            ))
        })
    }
}

#[async_trait]
impl TranscriptionEngine for RemoteWhisperEngine {
    async fn transcribe(
        &self,
        audio: &AudioData<'_>,
        language: &LanguageIso,
    ) -> Result<RawTranscript, TranscriptionError> {
        let bytes = match audio {
            AudioData::Bytes(b) => b.to_vec(),
            AudioData::Path(path) => tokio::fs::read(path.as_path())
                .await
                .map_err(|e| TranscriptionError::Engine(format!("read audio: {e}")))?,
        };

        let key = self.api_key()?;
        let lang = (!language.is_auto()).then(|| language.as_str());

        let form = reqwest::multipart::Form::new()
            .text("model", self.config.model.clone())
            .text("response_format", "json".to_owned())
            .part(
                "file",
                reqwest::multipart::Part::bytes(bytes)
                    .file_name("audio.bin")
                    .mime_str("application/octet-stream")
                    .map_err(|e| TranscriptionError::Engine(format!("mime: {e}")))?,
            );

        let form = match lang {
            Some(l) => form.text("language", l.to_owned()),
            None => form,
        };

        debug!(endpoint = %self.config.endpoint, "calling remote whisper");
        let resp = self
            .client
            .post(&self.config.endpoint)
            .bearer_auth(key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| TranscriptionError::Engine(format!("request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(TranscriptionError::Engine(format!(
                "remote whisper returned {status}: {body}"
            )));
        }

        let parsed: TranscribeResponse = resp
            .json()
            .await
            .map_err(|e| TranscriptionError::Engine(format!("bad response: {e}")))?;

        RawTranscript::new(parsed.text).map_err(|_| TranscriptionError::EmptyTranscript)
    }
}
