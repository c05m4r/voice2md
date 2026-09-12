//! Configuración de la CLI (TOML).

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use voice2md_core::FormattedOutputStyle;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub whisper: Whisper,
    pub output: Output,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub default_style: FormattedOutputStyle,
    pub default_language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Whisper {
    pub engine: String,
    pub model: String,
    pub model_path: PathBuf,
    pub api_url: String,
    pub api_key: Option<String>,
    pub api_key_env: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Output {
    pub out_dir: PathBuf,
    pub frontmatter: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            default_style: FormattedOutputStyle::Obsidian,
            default_language: "auto".to_owned(),
        }
    }
}

impl Default for Whisper {
    fn default() -> Self {
        Self {
            engine: "local".to_owned(),
            model: "base".to_owned(),
            model_path: PathBuf::from("models/ggml-base.bin"),
            api_url: "https://api.openai.com/v1/audio/transcriptions".to_owned(),
            api_key: None,
            api_key_env: "OPENAI_API_KEY".to_owned(),
        }
    }
}

impl Default for Output {
    fn default() -> Self {
        Self {
            out_dir: PathBuf::from("."),
            frontmatter: true,
        }
    }
}

impl Config {
    /// Candidatos de ubicación, en orden de prioridad.
    #[must_use]
    pub fn locations() -> Vec<PathBuf> {
        let mut v = Vec::new();
        if let Ok(env) = std::env::var("VOICE2MD_CONFIG") {
            v.push(PathBuf::from(env));
        }
        v.push(PathBuf::from("config.toml"));
        if let Some(home) = dirs_home() {
            v.push(home.join(".config/voice2md/config.toml"));
        }
        v
    }

    /// Carga la config desde la primera ubicación existente, o usa default.
    pub fn load() -> Result<Self> {
        for loc in Self::locations() {
            if loc.exists() {
                let raw = std::fs::read_to_string(&loc)
                    .with_context(|| format!("failed to read {}", loc.display()))?;
                let cfg: Config = toml::from_str(&raw)
                    .with_context(|| format!("failed to parse {}", loc.display()))?;
                return Ok(cfg);
            }
        }
        Ok(Self::default())
    }

    /// Devuelve el contenido de la plantilla de configuración.
    #[must_use]
    pub fn template() -> &'static str {
        r#"# voice2md configuration

[general]
default_style = "obsidian"    # obsidian | logseq | plain
default_language = "auto"     # BCP-47 or "auto"

[whisper]
engine = "local"              # local | remote
model = "base"                 # tiny | base | small | medium | large
model_path = "models/ggml-base.bin"

# remote engine settings
# api_url = "https://api.openai.com/v1/audio/transcriptions"
# api_key_env = "OPENAI_API_KEY"

[output]
out_dir = "."
frontmatter = true
"#
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
