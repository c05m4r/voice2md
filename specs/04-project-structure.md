# 04 — Estructura del Proyecto (Workspace / Crates)

> Organización física del repositorio. Sigue la regla de dependencia hexagonal:
> `core` no depende de ningún crate hermano salvo utilidades compartidas puras.

## 1. Layout del workspace

```
voice2md/
├── Cargo.toml                  # workspace (resolver = "2")
├── Cargo.lock
├── rust-toolchain.toml         # canal stable, edition 2024
├── rustfmt.toml
├── .github/
│   └── workflows/ci.yml
├── specs/                      # esta carpeta, fuente de verdad SDD
├── config/
│   └── config.example.toml
├── crates/
│   ├── core/                   # dominio + puertos + orquestador
│   ├── transcriber/            # adaptadores de TranscriptionEngine
│   ├── telegram-bot/           # adaptador inbound (binario)
│   └── cli/                    # adaptador inbound (binario)
└── tests/                      # tests de integración end-to-end (opcional)
```

## 2. Crate `core` (dominio + puertos)

Dependencias: solo crates puros (sin I/O de red, sin whisper).

| Dir | Contenido |
|-----|-----------|
| `src/lib.rs` | Re-export del dominio público |
| `src/domain/` | Entidades, value objects, enums, reglas de negocio |
| `src/domain/model.rs` | `Note`, `NoteId`, value objects |
| `src/domain/typestate.rs` | estados `New/Transcribed/Formatted/Failed` |
| `src/domain/errors.rs` | `DomainError` (thiserror) |
| `src/ports/` | Traits de puertos inbound/outbound |
| `src/ports/inbound.rs` | `ProcessAudioRequest`, `AudioInput` |
| `src/ports/outbound.rs` | `TranscriptionEngine`, `Formatter`, `Storage`, `AudioRepository` + sus errores |
| `src/application/` | Orquestador / application services |
| `src/application/orchestrator.rs` | `NoteOrchestrator` + `OrchestratorBuilder` |
| `src/application/pipeline.rs` | worker pool / `Job` / canales |

Deps clave: `thiserror`, `uuid`, `chrono`, `async-trait`, `tokio` (solo `sync`).

## 3. Crate `transcriber` (adaptadores outbound de STT)

| Dir | Contenido |
|-----|-----------|
| `src/lib.rs` | exporta `LocalWhisperEngine`, `RemoteWhisperEngine` |
| `src/local.rs` | `LocalWhisperEngine` (bindings C / `whisper-rs`, `spawn_blocking`) |
| `src/remote.rs` | `RemoteWhisperEngine` (API REST) |
| `src/model_loader.rs` | carga/caché de modelos `ggml` |
| `src/error.rs` | `TranscriptionError` (mapea a `core::ports::TranscriptionError`) |

Deps clave: `whisper-rs` (o crate C-bindings), `reqwest`, `bytes`, `tokio`.

Nota: `transcriber` depende de `core`, nunca al revés.

## 4. Crate `telegram-bot` (binario)

| Dir | Contenido |
|-----|-----------|
| `src/main.rs` | entrypoint, carga config, arranque bot |
| `src/bot/` | handlers de `teloxide` |
| `src/bot/handlers.rs` | `/start`, `/help`, `/style`, `/language`, voice |
| `src/bot/downloader.rs` | implementa `AudioRepository` (descarga via Telegram API) |
| `src/bot/delivery.rs` | implementa `Storage` (entrega como mensaje/documento) |
| `src/state/` | estado por usuario (estilo/idioma, `DashMap`) |
| `src/config.rs` | config del bot (token vía `TELOXIDE_TOKEN` o `Config`) |

Deps clave: `teloxide`, `tokio`, `reqwest`, `anyhow`, `tracing`, `trace` crates.

Depende de `core` y `transcriber`.

## 5. Crate `cli` (binario)

| Dir | Contenido |
|-----|-----------|
| `src/main.rs` | entrypoint, `clap` derive, dispatch |
| `src/commands/transcribe.rs` | subcomando `transcribe` |
| `src/commands/watch.rs` | subcomando `watch` (notify) |
| `src/commands/config.rs` | subcomando `config init/show` |
| `src/config.rs` | carga/validación TOML |
| `src/adapters/filestorage.rs` | implementa `Storage` (escritura `.md`) |

Deps clave: `clap` (derive), `tokio`, `notify`, `anyhow`, `tracing`, `figment`/`serde`+`toml`.

Depende de `core` y `transcriber`.

## 6. `Cargo.toml` del workspace

```toml
[workspace]
resolver = "3"
members = [
    "crates/core",
    "crates/transcriber",
    "crates/telegram-bot",
    "crates/cli",
]

[workspace.package]
edition = "2024"
rust-version = "1.85"          # MSRV: edition 2024 estable desde 1.85

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
thiserror = "1"
anyhow = "1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = "0.3"
serde = { version = "1", features = ["derive"] }
toml = "0.8"

[profile.release]
lto = "thin"
codegen-units = 1
```

## 7. Matriz de dependencias (resumen)

| Crate | Depende de |
|-------|-----------|
| `core` | solo crates puros (thiserror, uuid, chrono, tokio::sync, async-trait) |
| `transcriber` | `core`, whisper bindings, reqwest |
| `telegram-bot` | `core`, `transcriber`, teloxide |
| `cli` | `core`, `transcriber`, clap, notify |

Regla: **ningún** crate de infraestructura es dependencia de `core`. La flecha de
dependencia siempre apunta al dominio.

## 8. Tests

- Unit tests: dentro de cada crate, `#[cfg(test)] mod tests`.
- Tests de dominio: value objects y transiciones de typestate en `core`.
- Tests de puertos: mocks en `core/src/ports/../test_support` (o crate `test-support`).
- Integration: binario CLI (`--dry-run`) y bot con servidor telegram fake (opcional).

Comando de verificación previsto:
`cargo test --workspace && cargo clippy --workspace -- -D warnings && cargo fmt --check`.

---

*Documento vivo. Nuevo crate => actualizar `members`, la matriz de dependencias y
asegurar que respete la regla hexagonal.*
