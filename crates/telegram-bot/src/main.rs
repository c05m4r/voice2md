//! Bot de Telegram: recibe notas de voz, las transcribe y responde con Markdown.

mod audio_repo;
mod handlers;
mod state;

use std::sync::Arc;

use anyhow::Result;
use teloxide::dispatching::UpdateFilterExt;
use teloxide::dptree;
use teloxide::prelude::*;
use tracing::info;

use voice2md_core::{DefaultFormatter, NoteOrchestrator, OrchestratorBuilder};
use voice2md_transcriber::{RemoteWhisperConfig, RemoteWhisperEngine};

use crate::audio_repo::TelegramAudioRepository;
use crate::handlers::Command;
use crate::state::InMemUserState;

/// Máximo de caracteres por mensaje de Telegram.
const MAX_REPLY_CHARS: usize = 4096;

/// Dependencias compartidas por los handlers.
pub struct BotDeps {
    pub bot: Bot,
    pub orchestrator: Arc<NoteOrchestrator>,
    pub state: Arc<InMemUserState>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let bot = Bot::from_env();
    let token = bot.token().to_owned();

    let engine: Box<dyn voice2md_core::TranscriptionEngine> =
        match std::env::var("VOICE2MD_ENGINE").as_deref() {
            Ok("local") => {
                let model_path = std::env::var("VOICE2MD_MODEL_PATH")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|_| std::path::PathBuf::from("models/ggml-base.bin"));
                info!(?model_path, "using local whisper engine");
                let local = voice2md_transcriber::LocalWhisperConfig {
                    model_path,
                    ..Default::default()
                };
                Box::new(voice2md_transcriber::LocalWhisperEngine::new(local))
            }
            Ok("remote") | Err(_) => {
                info!("using remote whisper engine");
                Box::new(
                    RemoteWhisperEngine::new(RemoteWhisperConfig::default())
                        .expect("failed to build remote engine"),
                )
            }
            Ok(other) => anyhow::bail!("unknown VOICE2MD_ENGINE: {other} (local | remote)"),
        };

    let storage = Box::new(handlers::ChatStorage);
    let audio_repo = Box::new(TelegramAudioRepository::new(token));

    let orchestrator = OrchestratorBuilder::default()
        .engine(engine)
        .formatter(Box::new(DefaultFormatter))
        .storage(storage)
        .audio_repo(audio_repo)
        .build()
        .expect("failed to assemble orchestrator");

    let deps = Arc::new(BotDeps {
        bot: bot.clone(),
        orchestrator: Arc::new(orchestrator),
        state: Arc::new(InMemUserState::default()),
    });

    info!("telegram bot starting");

    let d_cmd = Arc::clone(&deps);
    let d_msg = Arc::clone(&deps);

    let handler = dptree::entry().branch(
        Update::filter_message()
            .branch(dptree::entry().filter_command::<Command>().endpoint(
                move |msg: Message, cmd: Command| {
                    let d = Arc::clone(&d_cmd);
                    async move { handlers::command_handler(d, msg, cmd).await }
                },
            ))
            .branch(dptree::endpoint(move |msg: Message| {
                let d = Arc::clone(&d_msg);
                async move { handlers::message_handler(d, msg).await }
            })),
    );

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
