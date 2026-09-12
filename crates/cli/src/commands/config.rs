//! Comando `config`: `init` y `show`.

use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use crate::config::Config;

#[derive(Args, Clone)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub sub: ConfigSub,
}

#[derive(Subcommand, Clone)]
pub enum ConfigSub {
    /// Genera la plantilla de configuración si no existe.
    Init,
    /// Muestra la configuración resuelta.
    Show,
}

pub fn run(args: ConfigArgs) -> Result<()> {
    match args.sub {
        ConfigSub::Init => init()?,
        ConfigSub::Show => show()?,
    }
    Ok(())
}

fn init() -> Result<()> {
    let loc = Config::locations()
        .into_iter()
        .find(|l| {
            // prioriza la ruta de env o el home del usuario
            l.exists() || l.is_absolute()
        })
        .unwrap_or_else(|| std::path::PathBuf::from("config.toml"));

    if loc.exists() {
        println!("config already exists at {}", loc.display());
        return Ok(());
    }

    if let Some(parent) = loc.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
    }
    std::fs::write(&loc, Config::template())
        .with_context(|| format!("cannot write {}", loc.display()))?;
    println!("wrote template config to {}", loc.display());
    Ok(())
}

fn show() -> Result<()> {
    let cfg = Config::load()?;
    let rendered = toml::to_string_pretty(&cfg).context("failed to serialize config")?;
    println!("{rendered}");
    Ok(())
}
