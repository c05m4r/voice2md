//! Comando `transcribe`: procesa un único audio.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Args;
use tracing::info;

use voice2md_core::{
    AudioFilePath, AudioInput, FormattedOutputStyle, LanguageIso, NoteOrchestrator,
    ProcessAudioRequest,
};

#[derive(Args, Clone)]
pub struct TranscribeArgs {
    /// Archivo de audio a procesar.
    pub path: PathBuf,

    /// Idioma BCP-47, o `auto`.
    #[arg(long)]
    pub language: Option<String>,

    /// Estilo de salida: obsidian | logseq | plain.
    #[arg(long)]
    pub style: Option<FormattedOutputStyle>,

    /// Título de la nota (por defecto derivado del transcript).
    #[arg(long)]
    pub title: Option<String>,

    /// No persistir; vuelca el Markdown a stdout.
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(args: TranscribeArgs, orchestrator: Arc<NoteOrchestrator>) -> Result<()> {
    let path = AudioFilePath::try_from_path(args.path).context("invalid audio path")?;

    let language = LanguageIso::new(args.language.as_deref().unwrap_or("auto"))?;
    let style = args.style.unwrap_or_default();

    let outcome = orchestrator
        .process(AudioInput::Local(path), language, style, None)
        .await
        .context("processing failed")?;

    info!(title = %outcome.note.title(), "note processed");

    if args.dry_run {
        print!("{}", outcome.markdown.as_str());
    } else {
        println!("{}", outcome.markdown.as_str());
    }

    Ok(())
}
