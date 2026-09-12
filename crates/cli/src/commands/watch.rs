//! Comando `watch`: vigila una carpeta y procesa audios nuevos.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Args;
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use tracing::{debug, info, warn};

use voice2md_core::{
    AudioFilePath, AudioInput, FormattedOutputStyle, LanguageIso, NoteOrchestrator,
    ProcessAudioRequest,
};

#[derive(Args, Clone)]
pub struct WatchArgs {
    /// Carpeta a vigilar.
    pub dir: PathBuf,

    /// Vigilar subdirectorios recursivamente.
    #[arg(long)]
    pub recursive: bool,

    /// Idioma BCP-47, o `auto`.
    #[arg(long)]
    pub language: Option<String>,

    /// Estilo de salida.
    #[arg(long)]
    pub style: Option<FormattedOutputStyle>,

    /// Debounce en milisegundos.
    #[arg(long, default_value_t = 1000)]
    pub debounce_ms: u64,
}

pub async fn run(args: WatchArgs, orchestrator: Arc<NoteOrchestrator>) -> Result<()> {
    let dir = args
        .dir
        .canonicalize()
        .context("cannot resolve watch directory")?;
    let language = LanguageIso::new(args.language.as_deref().unwrap_or("auto"))?;
    let style = args.style.unwrap_or_default();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<notify::Event>(128);

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(event) = res {
                let _ = tx.blocking_send(event);
            }
        },
        NotifyConfig::default(),
    )
    .context("failed to create watcher")?;

    let mode = if args.recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    watcher
        .watch(&dir, mode)
        .context("failed to watch directory")?;

    info!(dir = %dir.display(), "watching for audio files");

    let mut last_processed = std::time::Instant::now() - std::time::Duration::from_secs(3600);

    while let Some(event) = rx.recv().await {
        let now = std::time::Instant::now();
        if now.duration_since(last_processed).as_millis() < args.debounce_ms as u128 {
            continue;
        }
        last_processed = now;

        for path in event.paths {
            let Ok(audio) = AudioFilePath::try_from_path(path.clone()) else {
                debug!(path = %path.display(), "ignoring non-audio file");
                continue;
            };
            info!(path = %audio, "processing new audio");
            match orchestrator
                .process(AudioInput::Local(audio), language.clone(), style, None)
                .await
            {
                Ok(outcome) => info!(title = %outcome.note.title(), "note written"),
                Err(e) => warn!(error = %e, "failed to process audio"),
            }
        }
    }

    Ok(())
}
