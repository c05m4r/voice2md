//! Estado por usuario (estilo e idioma activos), en memoria.

use dashmap::DashMap;

use voice2md_core::{FormattedOutputStyle, LanguageIso};

/// Almacena preferencias por `chat_id`.
#[derive(Debug, Default)]
pub struct InMemUserState {
    style: DashMap<i64, FormattedOutputStyle>,
    language: DashMap<i64, LanguageIso>,
}

impl InMemUserState {
    #[must_use]
    pub fn style(&self, chat_id: i64) -> FormattedOutputStyle {
        self.style.get(&chat_id).map(|v| *v).unwrap_or_default()
    }

    pub fn set_style(&self, chat_id: i64, style: FormattedOutputStyle) {
        self.style.insert(chat_id, style);
    }

    #[must_use]
    pub fn language(&self, chat_id: i64) -> LanguageIso {
        self.language
            .get(&chat_id)
            .map(|v| v.clone())
            .unwrap_or_else(|| LanguageIso::new("auto").expect("auto is valid"))
    }

    pub fn set_language(&self, chat_id: i64, language: LanguageIso) {
        self.language.insert(chat_id, language);
    }
}
