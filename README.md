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
- Motor local: binario `whisper-cli` (whisper.cpp) + modelo `ggml-*.bin` + `ffmpeg`
- Motor remoto: clave de API (p. ej. OpenAI)

### Motor local (Ubuntu)

```bash
sudo apt install whisper.cpp ffmpeg
mkdir -p models
wget -O models/ggml-tiny.bin https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin
```

`whisper-cli` de apt solo lee WAV; los formatos comprimidos (ogg de Telegram, m4a…)
se convierten a WAV con `ffmpeg` automáticamente.

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
# Cargar variables del .env (exporta para que el proceso las vea)
set -a && source .env && set +a

export TELOXIDE_TOKEN=...
cargo run -p voice2md-telegram-bot
```

`source .env` a secas NO exporta las variables; sin `set -a` el bot falla con
`Cannot get the TELOXIDE_TOKEN env variable`. Alternativa directa:

```bash
TELOXIDE_TOKEN=tu_token cargo run -p voice2md-telegram-bot
```

Envía una nota de voz y recibirás el Markdown formateado por chat (o como documento `.md` si excede 4096 caracteres).

Comandos: `/start`, `/help`, `/style <estilo>`, `/language <LANG>`, `/allow <id>`, `/deny <id>`, `/mode <public|private>`.

### Control de acceso

El bot admite dos modos de acceso:

| Modo | Comportamiento |
|------|----------------|
| `public` | Cualquiera puede transcribir (default) |
| `private` | Solo usuarios en la whitelist transcriben |

Cualquier usuario **en la whitelist** puede administrar el bot:

- `/allow <id>` — autoriza a un usuario
- `/deny <id>` — revoca el acceso
- `/mode public|private` — cambia el modo

La whitelist y el modo se persisten en `whitelist.json` (ruta configurable) y sobreviven a los reinicios. Para arrancar con tus propios IDs ya autorizados, siémbralos con:

```bash
export VOICE2MD_WHITELIST=123456789,987654321
```

Formato de `whitelist.json`:

```json
{
  "mode": "private",
  "users": [
    987654321,
    123456789
  ]
}
```

> Usa `/whoami` en el bot para conocer tu ID de usuario.

> Nota: en modo `private`, las notas de voz de usuarios no autorizados se ignoran silenciosamente.

### Guardado en disco

Además del chat, el bot guarda cada transcripción como `.md` en disco, discriminada por usuario:

```
<out_dir>/
  user_<telegram_id>/
    <título_snake_case>_<timestamp>.md
  local/                 # notas vía CLI/archivo (sin user_id)
```

El nombre usa el título en `snake_case` más un timestamp UTC (`%Y%m%dT%H%M%SZ`), p. ej. `mi_nota_de_clase_20260912T153000Z.md`.

Configura el directorio raíz con:

```bash
export VOICE2MD_OUT_DIR=notes   # default
```

### Motor local vs remoto

El bot usa el motor **remoto** por defecto. Para usar Whisper local:

```bash
export VOICE2MD_ENGINE=local
export VOICE2MD_MODEL_PATH=models/ggml-tiny.bin
```

## Configuración

Variables de entorno (ver `.env.example`):

| Variable | Uso |
|----------|-----|
| `TELOXIDE_TOKEN` | Token del bot de Telegram |
| `VOICE2MD_ENGINE` | Motor del bot: `local` \| `remote` (default `remote`) |
| `VOICE2MD_MODEL_PATH` | Ruta al modelo local (default `models/ggml-tiny.bin`) |
| `OPENAI_API_KEY` | Clave para el motor remoto |
| `VOICE2MD_CONFIG` | Ruta a `config.toml` (opcional) |
| `VOICE2MD_OUT_DIR` | Directorio donde el bot guarda las notas (default `notes`) |
| `VOICE2MD_WHITELIST` | IDs iniciales autorizados, separados por coma (opcional) |
| `VOICE2MD_ACCESS_FILE` | Archivo que persiste whitelist y modo (default `whitelist.json`) |

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
