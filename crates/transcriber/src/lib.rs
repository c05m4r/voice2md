//! Adaptadores outbound de transcripción (Whisper).
//!
//! Implementan `voice2md_core::ports::outbound::TranscriptionEngine`:
//!
//! - `LocalWhisperEngine`: invoca el binario `whisper-cli` de `whisper.cpp`
//!   (proceso hijo, `spawn_blocking`). Reemplazable por bindings C (`whisper-rs`)
//!   sin tocar el dominio.
//! - `RemoteWhisperEngine`: llama a una API REST (p. ej. OpenAI Speech-to-Text).

pub mod local;
pub mod remote;

pub use local::{LocalWhisperConfig, LocalWhisperEngine};
pub use remote::{RemoteWhisperConfig, RemoteWhisperEngine};
