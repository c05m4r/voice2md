//! Motor Whisper local vía binario `whisper-cli` de `whisper.cpp`.
//!
//! `whisper-cli` (build apt de Ubuntu) solo lee WAV 16 kHz. Los formatos
//! comprimidos (ogg/opus, m4a, ...) se convierten primero con `ffmpeg`.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::process::Command;
use tracing::{debug, warn};

use voice2md_core::{
    AudioData, LanguageIso, RawTranscript, TranscriptionEngine, TranscriptionError,
};

/// Configuración del motor local.
#[derive(Debug, Clone)]
pub struct LocalWhisperConfig {
    /// Binario `whisper-cli` (o ruta si no está en `PATH`).
    pub binary: String,
    /// Ruta al modelo `ggml-*.bin`.
    pub model_path: PathBuf,
    /// Binario `ffmpeg` para convertir formatos comprimidos a WAV.
    pub ffmpeg: String,
    /// Argumentos extra pasados a `whisper-cli`.
    pub extra_args: Vec<String>,
}

impl Default for LocalWhisperConfig {
    fn default() -> Self {
        Self {
            binary: "whisper-cli".to_owned(),
            model_path: PathBuf::from("models/ggml-base.bin"),
            ffmpeg: "ffmpeg".to_owned(),
            extra_args: Vec::new(),
        }
    }
}

/// Motor local. Requiere `whisper-cli`, modelo descargado y `ffmpeg`.
#[derive(Debug, Clone)]
pub struct LocalWhisperEngine {
    config: LocalWhisperConfig,
}

impl LocalWhisperEngine {
    #[must_use]
    pub fn new(config: LocalWhisperConfig) -> Self {
        Self { config }
    }

    /// Convierte cualquier formato comprimido a WAV 16k mono.
    async fn to_wav(&self, input: &Path, wav: &Path) -> Result<(), TranscriptionError> {
        let out = Command::new(&self.config.ffmpeg)
            .arg("-y")
            .arg("-i")
            .arg(input)
            .arg("-ar")
            .arg("16000")
            .arg("-ac")
            .arg("1")
            .arg("-c:a")
            .arg("pcm_s16le")
            .arg("-f")
            .arg("wav")
            .arg(wav)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| {
                TranscriptionError::Engine(format!(
                    "failed to spawn {}: {e} (install ffmpeg to decode {})",
                    self.config.ffmpeg,
                    input.display()
                ))
            })?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(TranscriptionError::Engine(format!(
                "ffmpeg failed with {}: {}",
                out.status,
                stderr.trim()
            )));
        }
        Ok(())
    }

    /// Ejecuta `whisper-cli` sobre un WAV y devuelve el transcript.
    async fn run_whisper(
        &self,
        wav: &Path,
        language: &LanguageIso,
        out_base: &Path,
    ) -> Result<String, TranscriptionError> {
        let mut cmd = Command::new(&self.config.binary);
        cmd.arg("-m").arg(&self.config.model_path);
        cmd.arg("-f").arg(wav);
        cmd.arg("--output-txt");
        cmd.arg("--no-prints");
        cmd.arg("-of").arg(out_base);
        cmd.arg("-l").arg(if language.is_auto() { "auto" } else { language.as_str() });
        cmd.args(&self.config.extra_args);
        cmd.stdin(Stdio::null());
        cmd.stderr(Stdio::null());

        debug!(
            binary = %self.config.binary,
            model = %self.config.model_path.display(),
            "running whisper-cli"
        );
        let out = cmd.output().await.map_err(|e| {
            TranscriptionError::Engine(format!(
                "failed to spawn {}: {e} (install whisper.cpp)",
                self.config.binary
            ))
        })?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(TranscriptionError::Engine(format!(
                "{} exited with {}: {}",
                self.config.binary,
                out.status,
                stderr.trim()
            )));
        }

        let out_file = PathBuf::from(format!("{}.txt", out_base.display()));
        let text = tokio::fs::read_to_string(&out_file)
            .await
            .map_err(|e| TranscriptionError::Engine(format!("cannot read output: {e}")))?;

        let text = text.trim().to_owned();
        if text.is_empty() {
            return Err(TranscriptionError::EmptyTranscript);
        }
        Ok(text)
    }
}

fn temp_name(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("voice2md-{tag}-{nanos}"));
    debug!(path = %path.display(), "temp file");
    path
}

/// Borra los temporales al salir (también en error).
struct TempFiles(Vec<PathBuf>);

impl TempFiles {
    fn push(&mut self, p: PathBuf) {
        self.0.push(p);
    }
}

impl Drop for TempFiles {
    fn drop(&mut self) {
        for p in &self.0 {
            if let Err(e) = std::fs::remove_file(p) {
                warn!(path = %p.display(), error = %e, "failed to clean temp file");
            }
        }
    }
}

#[async_trait]
impl TranscriptionEngine for LocalWhisperEngine {
    async fn transcribe(
        &self,
        audio: &AudioData<'_>,
        language: &LanguageIso,
    ) -> Result<RawTranscript, TranscriptionError> {
        // Materializa el audio a un path (bytes -> fichero temporal).
        let mut temps = TempFiles(Vec::new());
        let input_path = match audio {
            AudioData::Path(p) => p.to_path_buf(),
            AudioData::Bytes(bytes) => {
                let f = temp_name("audio");
                tokio::fs::write(&f, bytes)
                    .await
                    .map_err(|e| TranscriptionError::Engine(format!("write temp: {e}")))?;
                temps.push(f.clone());
                f
            }
        };

        // Convierte a WAV si no lo es (whisper-cli solo lee WAV).
        let is_wav = input_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("wav"));

        let wav_path: PathBuf = if is_wav {
            input_path.clone()
        } else {
            let wav = temp_name("wav").with_extension("wav");
            self.to_wav(&input_path, &wav).await?;
            temps.push(wav.clone());
            wav
        };

        let out_base = temp_name("out");
        let result = self.run_whisper(&wav_path, language, &out_base).await;
        let _ = std::fs::remove_file(format!("{}.txt", out_base.display()));

        // El wav temporal se limpia en Drop de `temps`.
        let text = result?;
        RawTranscript::new(text).map_err(|_| TranscriptionError::EmptyTranscript)
    }
}
