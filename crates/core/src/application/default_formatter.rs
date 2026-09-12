//! Formateador por defecto: convierte el transcript crudo en Markdown
//! estructurado según el estilo (Obsidian / Logseq / Plain).

use async_trait::async_trait;

use crate::domain::errors::DomainError;
use crate::domain::value::{FormattedOutputStyle, MarkdownContent, NoteTitle, RawTranscript};
use crate::ports::outbound::{Formatter, NoteMeta};

/// Formateador incluido en el crate `core`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultFormatter;

/// Divide el transcript en párrafos por líneas/bloques en blanco.
fn paragraphs(transcript: &str) -> Vec<String> {
    transcript
        .split('\n')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Frontmatter YAML para estilos que lo soportan.
fn frontmatter(title: &NoteTitle, meta: &NoteMeta) -> String {
    let mut buf = String::from("---\n");
    buf.push_str(&format!("title: \"{}\"\n", title.as_str()));
    buf.push_str(&format!("date: {}\n", meta.date.to_rfc3339()));
    buf.push_str(&format!("language: {}\n", meta.language.as_str()));
    buf.push_str(&format!("source: {}\n", meta.source));
    if let Some(d) = meta.duration {
        buf.push_str(&format!("duration: {}\n", d.get()));
    }
    buf.push_str("tags: [voice-note]\n");
    buf.push_str("---\n\n");
    buf
}

/// Estructura el cuerpo en bullets (Logseq) o párrafos.
fn body(paras: &[String], style: FormattedOutputStyle) -> String {
    let mut buf = String::new();
    match style {
        FormattedOutputStyle::Logseq => {
            for p in paras {
                buf.push_str(&format!("- {p}\n"));
            }
        }
        _ => {
            for p in paras {
                buf.push_str(p);
                buf.push_str("\n\n");
            }
        }
    }
    buf
}

#[async_trait]
impl Formatter for DefaultFormatter {
    async fn format(
        &self,
        transcript: &RawTranscript,
        title: NoteTitle,
        style: FormattedOutputStyle,
        meta: NoteMeta,
    ) -> Result<MarkdownContent, DomainError> {
        let paras = paragraphs(transcript.as_str());
        let mut out = String::new();

        match style {
            FormattedOutputStyle::Plain => {
                out.push_str(&format!("# {}\n\n", title.as_str()));
            }
            _ => {
                out.push_str(&frontmatter(&title, &meta));
                out.push_str(&format!("# {}\n\n", title.as_str()));
            }
        }

        out.push_str(&body(&paras, style));
        MarkdownContent::new(out)
    }
}
