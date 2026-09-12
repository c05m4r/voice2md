//! CLI de `voice2md`: transcribe audios locales y los formatea a Markdown.

mod adapters;
mod commands;
mod config;

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::info;

use voice2md_core::{DefaultFormatter, NoteOrchestrator, OrchestratorBuilder};
use voice2md_transcriber::{
    LocalWhisperConfig, LocalWhisperEngine, RemoteWhisperConfig, RemoteWhisperEngine,
};

use crate::config::Config;

#[derive(Parser)]
#[command(name = "voice2md", version, about = "Voice notes -> Markdown")]
pub struct Cli {
    #[command(subcommand)]
    command: CliCommand,

    /// Nivel de log (error, warn, info, debug, trace).
    #[arg(long, global = true, default_value = "info")]
    log: String,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Procesa un archivo de audio local.
    Transcribe(commands::transcribe::TranscribeArgs),
    /// Observa una carpeta y procesa los audios nuevos.
    Watch(commands::watch::WatchArgs),
    /// Genera o muestra la configuración.
    Config(commands::config::ConfigArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cli.log)),
        )
        .init();

    let config = Config::load()?;

    match cli.command {
        CliCommand::Transcribe(args) => {
            let orchestrator = build_orchestrator(&config)?;
            commands::transcribe::run(args, orchestrator).await
        }
        CliCommand::Watch(args) => {
            let orchestrator = build_orchestrator(&config)?;
            commands::watch::run(args, orchestrator).await
        }
        CliCommand::Config(args) => commands::config::run(args),
    }
}

/// Construye el orquestador a partir de la configuración.
fn build_orchestrator(config: &Config) -> Result<Arc<NoteOrchestrator>> {
    let engine: Box<dyn voice2md_core::TranscriptionEngine> = match config.whisper.engine.as_str() {
        "local" => {
            let local = LocalWhisperConfig {
                model_path: config.whisper.model_path.clone(),
                ..Default::default()
            };
            Box::new(LocalWhisperEngine::new(local))
        }
        "remote" => {
            let remote = RemoteWhisperConfig {
                endpoint: config.whisper.api_url.clone(),
                api_key: config.whisper.api_key.clone(),
                api_key_env: config.whisper.api_key_env.clone(),
                model: config.whisper.model.clone(),
                ..Default::default()
            };
            Box::new(RemoteWhisperEngine::new(remote).context("failed to build remote engine")?)
        }
        other => anyhow::bail!("unknown whisper engine: {other} (expected 'local' or 'remote')"),
    };

    let storage = Box::new(adapters::FileStorage::new(config.output.out_dir.clone()));

    let orchestrator = OrchestratorBuilder::default()
        .engine(engine)
        .formatter(Box::new(DefaultFormatter))
        .storage(storage)
        .build()
        .context("failed to assemble orchestrator")?;

    info!(
        engine = config.whisper.engine,
        style = %config.general.default_style,
        "orchestrator ready"
    );
    Ok(Arc::new(orchestrator))
}
