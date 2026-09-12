//! Value objects (newtypes) con invariantes garantizadas por construcción.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::errors::DomainError;

/// Extensiones de audio soportadas.
pub const SUPPORTED_AUDIO_EXTENSIONS: &[&str] =
    &["wav", "mp3", "ogg", "m4a", "opus", "flac", "webm"];

/// Errores al construir value objects de audio.
#[derive(Debug, thiserror::Error)]
pub enum AudioFileError {
    #[error("path does not exist: {0}")]
    NotFound(String),
    #[error("unsupported audio extension: {0:?}")]
    UnsupportedExtension(Option<String>),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("path is not a file: {0}")]
    NotAFile(String),
}

/// Ruta absoluta, existente, con extensión de audio válida.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AudioFilePath(PathBuf);

impl AudioFilePath {
    /// Valida y normaliza (canonicaliza) una ruta a un fichero de audio.
    pub fn try_from_path(p: PathBuf) -> Result<Self, AudioFileError> {
        if !p.exists() {
            return Err(AudioFileError::NotFound(p.display().to_string()));
        }
        if !p.is_file() {
            return Err(AudioFileError::NotAFile(p.display().to_string()));
        }
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        if !SUPPORTED_AUDIO_EXTENSIONS.contains(&ext.as_deref().unwrap_or_default()) {
            return Err(AudioFileError::UnsupportedExtension(ext));
        }
        let canonical = p.canonicalize()?;
        Ok(Self(canonical))
    }

    #[must_use]
    pub fn as_path(&self) -> &std::path::Path {
        self.0.as_path()
    }

    #[must_use]
    pub fn to_path_buf(&self) -> PathBuf {
        self.0.clone()
    }

    #[must_use]
    pub fn extension(&self) -> Option<String> {
        self.0
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
    }
}

impl std::fmt::Display for AudioFilePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

/// Transcrito crudo, sin formato. Invariante: no vacío.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RawTranscript(String);

impl RawTranscript {
    pub fn new(s: impl Into<String>) -> Result<Self, DomainError> {
        let s = s.into();
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyTranscript);
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for RawTranscript {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Documento Markdown estructurado final.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MarkdownContent(String);

impl MarkdownContent {
    /// Invariante: no vacío y (salvo `plain`) empieza por encabezado o frontmatter.
    pub fn new(s: impl Into<String>) -> Result<Self, DomainError> {
        let content = s.into();
        if content.trim().is_empty() {
            return Err(DomainError::EmptyMarkdown);
        }
        Ok(Self(content))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

/// Título de nota, normalizado, longitud acotada.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoteTitle(String);

impl NoteTitle {
    pub const MAX_LEN: usize = 180;

    pub fn new(s: impl Into<String>) -> Result<Self, DomainError> {
        let raw = s.into();
        let cleaned: String = raw
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
                c if c.is_control() => ' ',
                c => c,
            })
            .collect();
        let trimmed = cleaned.trim().trim_matches('-').trim();
        if trimmed.is_empty() {
            return Err(DomainError::InvalidTitle(raw));
        }
        let mut out: String = trimmed.chars().take(Self::MAX_LEN).collect();
        if out.chars().count() > Self::MAX_LEN {
            out.truncate(Self::MAX_LEN);
        }
        Ok(Self(out.trim_end().to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NoteTitle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Duración de audio en segundos, estrictamente positiva.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AudioDurationSecs(u32);

impl AudioDurationSecs {
    pub fn new(secs: u32) -> Result<Self, DomainError> {
        if secs == 0 {
            return Err(DomainError::InvalidDuration);
        }
        Ok(Self(secs))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Idioma BCP-47 normalizado a minúsculas. `auto` permite detección.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LanguageIso(String);

impl LanguageIso {
    pub fn new(s: impl Into<String>) -> Result<Self, DomainError> {
        let v = s.into().trim().to_ascii_lowercase();
        if v.is_empty() {
            return Err(DomainError::InvalidLanguage);
        }
        Ok(Self(v))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_auto(&self) -> bool {
        self.0 == "auto"
    }
}

/// Formato de audio soportado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioFormat {
    Wav,
    Mp3,
    Ogg,
    M4a,
    Opus,
    Flac,
    Webm,
}

impl AudioFormat {
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "wav" => Some(Self::Wav),
            "mp3" => Some(Self::Mp3),
            "ogg" => Some(Self::Ogg),
            "m4a" => Some(Self::M4a),
            "opus" => Some(Self::Opus),
            "flac" => Some(Self::Flac),
            "webm" => Some(Self::Webm),
            _ => None,
        }
    }
}

/// Estilo de formateo de salida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FormattedOutputStyle {
    #[default]
    Obsidian,
    Logseq,
    Plain,
}

impl std::str::FromStr for FormattedOutputStyle {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "obsidian" => Ok(Self::Obsidian),
            "logseq" => Ok(Self::Logseq),
            "plain" => Ok(Self::Plain),
            other => Err(DomainError::InvalidStyle(other.to_owned())),
        }
    }
}

impl std::fmt::Display for FormattedOutputStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Obsidian => "obsidian",
            Self::Logseq => "logseq",
            Self::Plain => "plain",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_normalizes_invalid_chars_and_trims() {
        let t = NoteTitle::new("  Nota: con / caracteres * inválidos?  ").unwrap();
        assert_eq!(t.as_str(), "Nota- con - caracteres - inválidos");
    }

    #[test]
    fn title_rejects_empty() {
        assert!(NoteTitle::new("  ///  ").is_err());
    }

    #[test]
    fn title_truncates_to_max_len() {
        let long = "a".repeat(500);
        let t = NoteTitle::new(long).unwrap();
        assert!(t.as_str().chars().count() <= NoteTitle::MAX_LEN);
    }

    #[test]
    fn transcript_rejects_empty() {
        assert!(RawTranscript::new("   \n  ").is_err());
        assert!(RawTranscript::new("hola").is_ok());
    }

    #[test]
    fn markdown_rejects_empty() {
        assert!(MarkdownContent::new("").is_err());
    }

    #[test]
    fn language_normalizes_to_lowercase() {
        let l = LanguageIso::new("EN-US").unwrap();
        assert_eq!(l.as_str(), "en-us");
    }

    #[test]
    fn duration_must_be_positive() {
        assert!(AudioDurationSecs::new(0).is_err());
        assert!(AudioDurationSecs::new(42).is_ok());
    }

    #[test]
    fn style_roundtrips_via_display_and_fromstr() {
        for s in ["obsidian", "logseq", "plain"] {
            let style: FormattedOutputStyle = s.parse().unwrap();
            assert_eq!(style.to_string(), s);
        }
    }
}
