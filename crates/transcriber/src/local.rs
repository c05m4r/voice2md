//! Motor Whisper local vía binario `whisper-cli` de `whisper.cpp`.
//!
//! Estrategia: transcribir a un fichero temporal WAV y ejecutar el binario en un
//! proceso hijo. La ejecución es síncrona (esperamos salida) por lo que se
//! despacha en `spawn_blocking` para no bloquear el runtime de tokio.

use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::Command;
use tracing::debug;

use voice2md_core::{
    AudioData, LanguageIso, RawTranscript, TranscriptionEngine, TranscriptionError,
};

/// Configuración del motor local.
#[derive(Debug, Clone)]
pub struct LocalWhisperConfig {
    /// Ruta al binario `whisper-cli` (o nombre si está en `PATH`).
    pub binary: String,
    /// Ruta al modelo `ggml-*.bin`.
    pub model_path: PathBuf,
    /// Argumentos extra pasados al binario.
    pub extra_args: Vec<String>,
}

impl Default for LocalWhisperConfig {
    fn default() -> Self {
        Self {
            binary: "whisper-cli".to_owned(),
            model_path: PathBuf::from("models/ggml-base.bin"),
            extra_args: Vec::new(),
        }
    }
}

/// Transcripción local. Requiere `whisper-cli` instalado y modelo descargado.
#[derive(Debug, Clone)]
pub struct LocalWhisperEngine {
    config: LocalWhisperConfig,
}

impl LocalWhisperEngine {
    #[must_use]
    pub fn new(config: LocalWhisperConfig) -> Self {
        Self { config }
    }

    async fn run(
        &self,
        path: &std::path::Path,
        language: &LanguageIso,
    ) -> Result<String, TranscriptionError> {
        let binary = &self.config.binary;
        let mut cmd = Command::new(binary);
        cmd.arg("-m").arg(&self.config.model_path);
        cmd.arg("-f").arg(path);
        cmd.arg("--output-txt");
        cmd.arg("--no-prints");
        if !language.is_auto() {
            cmd.arg("-l").arg(language.as_str());
        }
        cmd.args(&self.config.extra_args);
        cmd.stdin(Stdio::null());
        cmd.stderr(Stdio::null());

        debug!(binary = %binary, "running whisper-cli");
        let out = cmd
            .output()
            .await
            .map_err(|e| TranscriptionError::Engine(format!("failed to spawn whisper-cli: {e}")))?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(TranscriptionError::Engine(format!(
                "whisper-cli exited with {}: {}",
                out.status,
                stderr.trim()
            )));
        }

        let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if text.is_empty() {
            return Err(TranscriptionError::EmptyTranscript);
        }
        Ok(text)
    }
}

#[async_trait]
impl TranscriptionEngine for LocalWhisperEngine {
    async fn transcribe(
        &self,
        audio: &AudioData<'_>,
        language: &LanguageIso,
    ) -> Result<RawTranscript, TranscriptionError> {
        match audio {
            AudioData::Path(path) => {
                let text = self.run(path.as_path(), language).await?;
                RawTranscript::new(text).map_err(|_| TranscriptionError::EmptyTranscript)
            }
            AudioData::Bytes(_) => Err(TranscriptionError::UnsupportedInput),
        }
    }
}
