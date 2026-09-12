//! Descarga de audios desde la API de Telegram (adaptador `AudioRepository`).

use async_trait::async_trait;
use serde_json::Value;
use tracing::debug;

use voice2md_core::{AudioFetchError, AudioRepository, RemoteAudioRef};

const MAX_SIZE_BYTES: u64 = 25 * 1024 * 1024;

/// Resuelve `RemoteAudioRef::Telegram` a bytes usando la API de ficheros de Telegram.
#[derive(Debug, Clone)]
pub struct TelegramAudioRepository {
    client: reqwest::Client,
    token: String,
}

impl TelegramAudioRepository {
    #[must_use]
    pub fn new(token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
        }
    }

    async fn file_path(&self, file_id: &str) -> Result<String, AudioFetchError> {
        let url = format!("https://api.telegram.org/bot{}/getFile", self.token);
        let resp = self
            .client
            .get(&url)
            .query(&[("file_id", file_id)])
            .send()
            .await
            .map_err(|e| AudioFetchError::Download(e.to_string()))?;

        let json: Value = resp
            .json()
            .await
            .map_err(|e| AudioFetchError::Download(e.to_string()))?;

        if json.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(AudioFetchError::NotFound);
        }

        json.get("result")
            .and_then(|r| r.get("file_path"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or(AudioFetchError::NotFound)
    }
}

#[async_trait]
impl AudioRepository for TelegramAudioRepository {
    async fn fetch(&self, r#ref: &RemoteAudioRef) -> Result<Vec<u8>, AudioFetchError> {
        let RemoteAudioRef::Telegram { file_id, .. } = r#ref;

        let file_path = self.file_path(file_id).await?;
        let url = format!(
            "https://api.telegram.org/file/bot{}/{}",
            self.token, file_path
        );
        debug!(url = %url, "downloading voice note");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AudioFetchError::Download(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(AudioFetchError::NotFound);
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| AudioFetchError::Download(e.to_string()))?;

        if bytes.len() as u64 > MAX_SIZE_BYTES {
            return Err(AudioFetchError::TooLarge(
                bytes.len() as u64,
                MAX_SIZE_BYTES,
            ));
        }

        Ok(bytes.to_vec())
    }
}
