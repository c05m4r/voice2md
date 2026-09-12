//! Handlers del bot: comandos y notas de voz.

use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;

use teloxide::prelude::*;
use teloxide::types::InputFile;
use teloxide::utils::command::BotCommands;
use tracing::{debug, warn};

use voice2md_core::{
    AudioInput, FormattedOutputStyle, LanguageIso, MarkdownContent, ProcessAudioRequest,
    RemoteAudioRef,
};

use crate::BotDeps;
use crate::MAX_REPLY_CHARS;
use crate::access::AccessMode;

type HandlerResult = Result<(), Box<dyn Error + Send + Sync>>;

/// Comandos del bot.
#[derive(BotCommands, Clone, Debug, PartialEq)]
#[command(rename_rule = "lowercase")]
pub enum Command {
    #[command(description = "mensaje de bienvenida")]
    Start,
    #[command(description = "lista de comandos")]
    Help,
    #[command(description = "cambia el estilo de salida (obsidian|logseq|plain)")]
    Style(String),
    #[command(description = "fija el idioma por defecto (BCP-47 o auto)")]
    Language(String),
    #[command(description = "permite a un usuario usar el bot (solo whitelist)")]
    Allow(String),
    #[command(description = "revoca el acceso de un usuario (solo whitelist)")]
    Deny(String),
    #[command(description = "modo del bot: public o private (solo whitelist)")]
    Mode(String),
    #[command(description = "muestra tu ID de usuario de Telegram")]
    Whoami,
}

pub async fn command_handler(deps: Arc<BotDeps>, msg: Message, cmd: Command) -> HandlerResult {
    let chat_id: i64 = chat_id_of(&msg);
    match cmd {
        Command::Start | Command::Help => {
            let text = concat!(
                "🎙 voice2md: envíame una nota de voz y la convierto en Markdown.\n\n",
                "/style <obsidian|logseq|plain> — cambia el estilo\n",
                "/language <LANG> — fija el idioma (auto para detección)\n",
                "/allow <id> — permite a un usuario (whitelist)\n",
                "/deny <id> — revoca acceso (whitelist)\n",
                "/mode <public|private> — modo de acceso (whitelist)\n",
                "/whoami — muestra tu ID de usuario\n",
                "/help — este mensaje"
            );
            deps.bot.send_message(msg.chat.id, text).send().await?;
        }
        Command::Style(raw) => match FormattedOutputStyle::from_str(&raw) {
            Ok(style) => {
                deps.state.set_style(chat_id, style);
                deps.bot
                    .send_message(msg.chat.id, format!("Estilo actualizado: {raw}"))
                    .send()
                    .await?;
            }
            Err(_) => {
                deps.bot
                    .send_message(
                        msg.chat.id,
                        "Estilo inválido. Usa obsidian, logseq o plain.",
                    )
                    .send()
                    .await?;
            }
        },
        Command::Language(raw) => match LanguageIso::new(raw.clone()) {
            Ok(lang) => {
                deps.state.set_language(chat_id, lang);
                deps.bot
                    .send_message(msg.chat.id, format!("Idioma actualizado: {raw}"))
                    .send()
                    .await?;
            }
            Err(_) => {
                deps.bot
                    .send_message(msg.chat.id, "Idioma inválido.")
                    .send()
                    .await?;
            }
        },
        Command::Allow(raw) => {
            let user_id = user_id_of(&msg);
            if !deps.access.is_manager(user_id) {
                deny_admin(&deps, &msg).await?;
                return Ok(());
            }
            match raw.parse::<i64>() {
                Ok(target) => match deps.access.allow(target)? {
                    true => {
                        deps.bot
                            .send_message(msg.chat.id, format!("Usuario {target} autorizado."))
                            .send()
                            .await?;
                    }
                    false => {
                        deps.bot
                            .send_message(
                                msg.chat.id,
                                format!("El usuario {target} ya estaba autorizado."),
                            )
                            .send()
                            .await?;
                    }
                },
                Err(_) => {
                    deps.bot
                        .send_message(msg.chat.id, "Id inválido. Usa /allow <id numérico>.")
                        .send()
                        .await?;
                }
            }
        }
        Command::Deny(raw) => {
            let user_id = user_id_of(&msg);
            if !deps.access.is_manager(user_id) {
                deny_admin(&deps, &msg).await?;
                return Ok(());
            }
            match raw.parse::<i64>() {
                Ok(target) => match deps.access.deny(target)? {
                    true => {
                        deps.bot
                            .send_message(msg.chat.id, format!("Usuario {target} revocado."))
                            .send()
                            .await?;
                    }
                    false => {
                        deps.bot
                            .send_message(msg.chat.id, format!("El usuario {target} no estaba."))
                            .send()
                            .await?;
                    }
                },
                Err(_) => {
                    deps.bot
                        .send_message(msg.chat.id, "Id inválido. Usa /deny <id numérico>.")
                        .send()
                        .await?;
                }
            }
        }
        Command::Mode(raw) => {
            let user_id = user_id_of(&msg);
            if !deps.access.is_manager(user_id) {
                deny_admin(&deps, &msg).await?;
                return Ok(());
            }
            match AccessMode::from_str(&raw) {
                Ok(mode) => {
                    deps.access.set_mode(mode)?;
                    let label = match mode {
                        AccessMode::Public => "public",
                        AccessMode::Private => "private",
                    };
                    deps.bot
                        .send_message(msg.chat.id, format!("Modo actualizado: {label}"))
                        .send()
                        .await?;
                }
                Err(_) => {
                    deps.bot
                        .send_message(msg.chat.id, "Modo inválido. Usa /mode public o /mode private.")
                        .send()
                        .await?;
                }
            }
        }
        Command::Whoami => {
            let user_id = user_id_of(&msg);
            deps.bot
                .send_message(msg.chat.id, format!("Tu ID de usuario: {user_id}"))
                .send()
                .await?;
        }
    }
    Ok(())
}

/// Respuesta para un usuario sin permiso de administración.
async fn deny_admin(deps: &BotDeps, msg: &Message) -> HandlerResult {
    deps.bot
        .send_message(msg.chat.id, "No tienes permiso para administrar el bot.")
        .send()
        .await?;
    Ok(())
}

pub async fn message_handler(deps: Arc<BotDeps>, msg: Message) -> HandlerResult {
    let Some(voice) = msg.voice() else {
        return Ok(());
    };

    let user_id = user_id_of(&msg);
    if !deps.access.is_allowed(user_id) {
        debug!(user_id, "voice note from unauthorized user ignored");
        return Ok(());
    }

    let chat_id = chat_id_of(&msg);
    let style = deps.state.style(chat_id);
    let language = deps.state.language(chat_id);
    let duration_hint = Some(voice.duration.seconds());

    let r#ref = RemoteAudioRef::Telegram {
        file_id: voice.file.id.to_string(),
        chat_id,
        user_id,
    };

    debug!(
        chat_id,
        duration = voice.duration.seconds(),
        "received voice note"
    );
    deps.bot
        .send_message(msg.chat.id, "Procesando nota de voz…")
        .send()
        .await?;

    let outcome = match deps
        .orchestrator
        .process(AudioInput::Remote(r#ref), language, style, duration_hint)
        .await
    {
        Ok(o) => o,
        Err(e) => {
            warn!(error = %e, "processing failed");
            deps.bot
                .send_message(msg.chat.id, friendly_error(&e))
                .send()
                .await?;
            return Ok(());
        }
    };

    deliver(&deps.bot, msg.chat.id, &outcome.markdown).await?;
    Ok(())
}

/// Envía el Markdown por chat; si excede el límite, como documento `.md`.
async fn deliver(bot: &Bot, chat_id: ChatId, content: &MarkdownContent) -> HandlerResult {
    let text = content.as_str();
    if text.chars().count() <= MAX_REPLY_CHARS {
        bot.send_message(chat_id, text).send().await?;
    } else {
        let bytes = text.as_bytes().to_vec();
        bot.send_document(chat_id, InputFile::memory(bytes).file_name("note.md"))
            .send()
            .await?;
    }
    Ok(())
}

fn chat_id_of(msg: &Message) -> i64 {
    msg.chat.id.to_string().parse::<i64>().unwrap_or_default()
}

fn user_id_of(msg: &Message) -> i64 {
    msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0)
}

fn friendly_error(e: &voice2md_core::DomainError) -> String {
    use voice2md_core::DomainError as E;
    match e {
        E::AudioTooLong(d, _) => format!("El audio supera el límite ({d}s)."),
        E::AudioTooLarge(_, _) => "El archivo supera 25 MB.".to_owned(),
        E::EmptyTranscript | E::Transcription(_) => {
            "No pude transcribir el audio (sin voz detectable).".to_owned()
        }
        E::AudioFetch(_) => "No pude descargar el audio.".to_owned(),
        other => format!("Error al procesar: {other}"),
    }
}
