# voice2md

Convierte notas de voz y grabaciones de clases en **Markdown estructurado**, listo para editar en Obsidian o Logseq.

Flujo: `audio → Whisper → transcripción → formateo → Markdown`.

Escrito en Rust (edition 2024) con arquitectura hexagonal, type-driven design y patrón typestate. Ver `specs/` para la especificación completa (SDD).

## Crates

| Crate | Rol |
|-------|-----|
| `core` | Dominio, value objects, puertos (traits) y orquestador |
| `transcriber` | Motores Whisper: local (whisper-cli) y remoto (API REST) |
| `cli` | Binario `voice2md`: transcribe archivos o vigila carpetas |
| `telegram-bot` | Binario `voice2md-bot`: recibe notas de voz por Telegram |

## Requisitos

- Rust (edition 2024)
- Motor local: binario `whisper-cli` (whisper.cpp) + modelo `ggml-*.bin` en `PATH`
- Motor remoto: clave de API (p. ej. OpenAI)

## Instalación

```bash
cargo build --release
```

Binarios en `target/release/`: `voice2md` y `voice2md-bot`.

## Uso — CLI

```bash
voice2md --help
Voice notes -> Markdown

Usage: voice2md [OPTIONS] <COMMAND>

Commands:
  transcribe  Procesa un archivo de audio local
  watch       Observa una carpeta y procesa los audios nuevos
  config      Genera o muestra la configuración
  help        Print this message or the help of the given subcommand(s)

Options:
      --log <LOG>  Nivel de log (error, warn, info, debug, trace) [default: info]
  -h, --help       Print help
  -V, --version    Print version
  
---

voice2md transcribe --help
Procesa un archivo de audio local

Usage: voice2md transcribe [OPTIONS] <PATH>

Arguments:
  <PATH>  Archivo de audio a procesar

Options:
      --language <LANGUAGE>  Idioma BCP-47, o `auto`
      --style <STYLE>        Estilo de salida: obsidian | logseq | plain
      --title <TITLE>        Título de la nota (por defecto derivado del transcript)
      --dry-run              No persistir; vuelca el Markdown a stdout
      --log <LOG>            Nivel de log (error, warn, info, debug, trace) [default: info]
  -h, --help                 Print help

---

voice2md watch --help
Observa una carpeta y procesa los audios nuevos

Usage: voice2md watch [OPTIONS] <DIR>

Arguments:
  <DIR>  Carpeta a vigilar

Options:
      --recursive                  Vigilar subdirectorios recursivamente
      --language <LANGUAGE>        Idioma BCP-47, o `auto`
      --style <STYLE>              Estilo de salida
      --debounce-ms <DEBOUNCE_MS>  Debounce en milisegundos [default: 1000]
      --log <LOG>                  Nivel de log (error, warn, info, debug, trace) [default: info]
  -h, --help                       Print help

---

voice2md config --help
Genera o muestra la configuración

Usage: voice2md config [OPTIONS] <COMMAND>

Commands:
  init  Genera la plantilla de configuración si no existe
  show  Muestra la configuración resuelta
  help  Print this message or the help of the given subcommand(s)

Options:
      --log <LOG>  Nivel de log (error, warn, info, debug, trace) [default: info]
  -h, --help       Print help

```

### TLDR

```bash
# Transcribir un audio a Markdown
voice2md transcribe clase.wav --language es --style obsidian

# Vigilar una carpeta y procesar audios nuevos
voice2md watch ./grabaciones --recursive

# Generar/ver configuración
voice2md config init
voice2md config show
```

Estilos: `obsidian` | `logseq` | `plain`. La salida se escribe como `<título>.md`; con `--dry-run` se vuelca a stdout.

## Uso — Bot de Telegram

```bash
export TELOXIDE_TOKEN=...
cargo run -p voice2md-telegram-bot
```

Envía una nota de voz y recibirás el Markdown formateado por chat (o como documento `.md` si excede 4096 caracteres).

Comandos: `/start`, `/help`, `/style <estilo>`, `/language <LANG>`.

## Configuración

Variables de entorno (ver `.env.example`):

| Variable | Uso |
|----------|-----|
| `TELOXIDE_TOKEN` | Token del bot de Telegram |
| `OPENAI_API_KEY` | Clave para el motor remoto (nombre configurable) |
| `VOICE2MD_CONFIG` | Ruta a `config.toml` (opcional) |

La CLI también lee `config.toml` (local o `~/.config/voice2md/config.toml`). Ejemplo en `config/config.example.toml`.

## Arquitectura

```
CLI / Bot  ──>  ProcessAudioRequest (inbound port)
                        │
                  NoteOrchestrator
                        │
         TranscriptionEngine ──> Whisper (local/remote)
         Formatter            ──> Markdown (obsidian/logseq/plain)
         Storage              ──> disco / chat
```

El dominio (`core`) no depende de Telegram, Whisper ni red. Los motores y canales de entrada son adaptadores intercambiables detrás de traits.

## Desarrollo

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```
