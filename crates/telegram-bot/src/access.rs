//! Control de acceso del bot: whitelist persistente y modo público/privado.
//!
//! - Modo `public`: cualquiera puede transcribir.
//! - Modo `private`: solo usuarios en la whitelist transcriben.
//! - Cualquier usuario en la whitelist puede `/allow`, `/deny` y `/mode`.
//!
//! El estado se persiste en un archivo JSON (`whitelist.json` por defecto).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::RwLock;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AccessMode {
    #[default]
    Public,
    Private,
}

impl FromStr for AccessMode {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "public" => Ok(Self::Public),
            "private" => Ok(Self::Private),
            _ => Err("debe ser public o private"),
        }
    }
}

/// Forma persistida del estado de acceso.
#[derive(Debug, Serialize, Deserialize)]
struct PersistedState {
    mode: AccessMode,
    users: Vec<i64>,
}

#[derive(Debug)]
struct State {
    mode: AccessMode,
    users: HashSet<i64>,
}

/// Control de acceso con persistencia en disco.
#[derive(Debug)]
pub struct AccessControl {
    path: PathBuf,
    state: RwLock<State>,
}

impl AccessControl {
    /// Carga desde `path`; si no existe, arranca público con la whitelist
    /// sembrada por `seed` (IDs de Telegram separados por coma).
    pub fn load(path: impl AsRef<Path>, seed: Option<&str>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut state = match fs::read_to_string(&path) {
            Ok(raw) => {
                let persisted: PersistedState = serde_json::from_str(&raw)
                    .with_context(|| format!("parsear {}", path.display()))?;
                State {
                    mode: persisted.mode,
                    users: persisted.users.into_iter().collect(),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State {
                mode: AccessMode::Public,
                users: HashSet::new(),
            },
            Err(e) => return Err(e).with_context(|| format!("leer {}", path.display())),
        };

        let mut seeded = false;
        if let Some(seed) = seed {
            for raw_id in seed.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                match raw_id.parse::<i64>() {
                    Ok(id) => {
                        if state.users.insert(id) {
                            seeded = true;
                        }
                    }
                    Err(_) => warn!(raw_id, "VOICE2MD_WHITELIST: id no numérico ignorado"),
                }
            }
        }

        let access = Self {
            path,
            state: RwLock::new(state),
        };
        if seeded {
            access.persist()?;
        }
        Ok(access)
    }

    #[must_use]
    pub fn mode(&self) -> AccessMode {
        self.state.read().unwrap().mode
    }

    #[must_use]
    pub fn is_allowed(&self, user_id: i64) -> bool {
        let state = self.state.read().unwrap();
        state.mode == AccessMode::Public || state.users.contains(&user_id)
    }

    #[must_use]
    pub fn is_manager(&self, user_id: i64) -> bool {
        self.state.read().unwrap().users.contains(&user_id)
    }

    pub fn allow(&self, user_id: i64) -> Result<bool> {
        let mut state = self.state.write().unwrap();
        let inserted = state.users.insert(user_id);
        drop(state);
        if inserted {
            self.persist()?;
        }
        Ok(inserted)
    }

    pub fn deny(&self, user_id: i64) -> Result<bool> {
        let mut state = self.state.write().unwrap();
        let removed = state.users.remove(&user_id);
        drop(state);
        if removed {
            self.persist()?;
        }
        Ok(removed)
    }

    pub fn set_mode(&self, mode: AccessMode) -> Result<()> {
        {
            let mut state = self.state.write().unwrap();
            state.mode = mode;
        }
        self.persist()
    }

    #[must_use]
    pub fn users(&self) -> Vec<i64> {
        let state = self.state.read().unwrap();
        let mut v: Vec<i64> = state.users.iter().copied().collect();
        v.sort_unstable();
        v
    }

    fn persist(&self) -> Result<()> {
        let state = self.state.read().unwrap();
        let mut users: Vec<i64> = state.users.iter().copied().collect();
        users.sort_unstable();
        let persisted = PersistedState {
            mode: state.mode,
            users,
        };
        let raw = serde_json::to_string_pretty(&persisted)?;
        fs::write(&self.path, raw)
            .with_context(|| format!("escribir {}", self.path.display()))?;
        debug!(path = %self.path.display(), "access state persisted");
        Ok(())
    }
}
