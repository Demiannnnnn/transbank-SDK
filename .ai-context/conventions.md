# Convenciones — transbank-SDK

## 1. Idioma

| Qué | Idioma |
|:--|:--|
| Código: identificadores, archivos, commits, comentarios, mensajes de `Error`, rustdoc | Inglés |
| Términos del dominio | En inglés en el código (`ticket`, `commerce_code`, `installments`); nombres de terminal se mantienen (`PosIntegrado`, `PosAutoservicio`) |
| Mensajes de los códigos de respuesta (`ResponseCode::message`) | Español, tal como los entrega Transbank (protocol-spec §9) |
| Salida de la CLI y del simulador para humanos | Español de Chile (tuteo); `--json` con claves en inglés |
| Documentación (`docs/`, `.ai-context/`, `tasks/`, reportes) | Español |
| README de cada crate (crates.io) | Inglés, con enlace al README del repo |

## 2. Git y ramas (reglas del dueño — obligatorias)

| Rama | Quién | Regla |
|:--|:--|:--|
| `main` | **Nadie** (ni agentes ni orquestador) | Prohibido: checkout para commitear, commit, merge, push, PR, rebase, borrar. |
| `develop` | **Nadie** (ni agentes ni orquestador) | Igual que `main`. El dueño integra `develop-AGENTS → develop` manualmente. |
| `develop-AGENTS` | Solo el **orquestador** (Claude) | Rama de integración. Recibe merges `--no-ff` de `dev/*` validados. |
| `dev/*` | Agentes y orquestador | Libres. Ej.: `dev/infra-01-workspace`, `dev/proto-01-framing`, `dev/fix-<tema>`, `dev/docs-<tema>`. |

- Cada tarea se trabaja en `dev/<id-en-minúsculas>-<tema>` creada desde `develop-AGENTS` (el orquestador la crea y la deja checkouteada).
- Commits: Conventional Commits en inglés con scope = módulo o crate, y referencia a la tarea en el cuerpo:
  `feat(protocol): add streaming frame decoder` + `Refs: TASK-PROTO-01`.
  Tipos: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `build`, `ci`, `perf`.
- Commits pequeños y coherentes; nunca commitear `target/`.

## 3. Rust

- Edición 2024, MSRV 1.85. `cargo fmt` por defecto.
- **Lints del workspace** (`[workspace.lints]`, cada crate con `[lints] workspace = true`):
  - rust: `unsafe_code = "forbid"`, `missing_docs = "deny"`, `rust_2018_idioms = "deny"`, `unreachable_pub = "warn"`.
  - clippy: `all = "deny"`, `pedantic = "warn"` (prioridad -1), `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `indexing_slicing`, `float_arithmetic`, `dbg_macro`, `print_stdout`, `print_stderr` = `"deny"`; `module_name_repetitions = "allow"` (único allow, decidido aquí).
  - `clippy.toml`: `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests`, `allow-indexing-slicing-in-tests` = `true`.
  - Los binarios (CLI, simulador) pueden imprimir: en sus `main.rs`/módulos de salida usan `std::io::Write` sobre `stdout().lock()` (no `println!`), así los lints valen igual en todo el workspace.
- Errores: `thiserror` en la librería; nada de `Box<dyn Error>` en la API salvo `Error::Transport`. Los binarios convierten a mensaje y código de salida (`ExitCode`).
- Sin `static mut`, sin `lazy_static`; constantes `const`.
- Visibilidad mínima: `pub(crate)` por defecto; `pub` solo lo de `public-api.md`.
- Rustdoc: cada ítem público con descripción, `# Errors` en funciones que devuelven `Result`, y ejemplo en los principales (API-06).
- Nombres: tipos `PascalCase`, funciones `snake_case`, constantes `SCREAMING_SNAKE`. Códigos de función como `const` en `protocol` (`FC_SALE: &str = "0200"`, `FC_SALE_RESPONSE: &str = "0210"`…), nunca literales repartidos.
- Ninguna función de más de ~60 líneas; parsers con helpers (`fields.rs`: `field_str`, `field_u32`, `field_date`…).

## 4. Features

| Feature | Default | Activa |
|:--|:--|:--|
| `serial` | sí | `serialport`, módulo `serial`, `PosIntegrado::open/autoconnect` |
| `serde` | no | `Serialize` en respuestas y tipos; `Deserialize` en requests y tipos de entrada |
| `tracing` | no | Eventos `tracing` en el enlace (security-model §3) |
| `tokio` | no | API async (ASYNC-01) |

## 5. Tests

| Capa | Dónde | Cómo |
|:--|:--|:--|
| protocol (LRC, tramas, decoder, campos, tipos) | `#[cfg(test)] mod tests` junto al código | Vectores de protocol-spec §2.3; `proptest` (PROTO-04) |
| message (requests/respuestas) | `crates/transbank-pos/tests/messages.rs` + `tests/fixtures/*.txt` | Un fixture por respuesta de la spec (tramas reales de los SDKs, con comentario de origen) |
| link | `#[cfg(test)]` en `link.rs` | `MemoryTransport` + `ManualClock`: ACK, NAK, reintentos, timeouts, basura, LRC malo, 0900, listas |
| client | `crates/transbank-pos/tests/client_*.rs` | Contra `transbank-pos-sim` (`SimTransport`) — QA-02 |
| simulator | en el crate sim | Unitarios + TCP en `127.0.0.1:0` |
| CLI | `crates/transbank-pos-cli/tests/` | Ejecuta el binario (`env!("CARGO_BIN_EXE_tbk-pos")`) con `--sim` |

- Deterministas (QA-03): nada de `thread::sleep`; tiempo con `ManualClock`; TCP solo en loopback con puerto 0.
- Un test por comportamiento, nombre descriptivo: `sale_is_not_resent_after_ack_when_response_times_out`.
- Fixtures: archivos de texto, una trama por línea, `#` para comentarios (origen).

## 6. Comandos raíz

```bash
cargo build --workspace
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy -p transbank-pos --no-default-features -- -D warnings
cargo fmt --all
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
cargo run -p transbank-pos-sim -- --listen 127.0.0.1:7777
cargo run -p transbank-pos-cli -- --tcp 127.0.0.1:7777 poll
```

## 7. Versionado

SemVer. Hasta `1.0` los cambios incompatibles suben el minor. `CHANGELOG.md` (formato Keep a Changelog) en la raíz desde DOCS-01. Los tres crates comparten versión (`[workspace.package] version`).

## 8. Reporte de tarea

Al terminar (o bloquearse), el agente actualiza **solo** la línea `Estado` y la sección `## Reporte` del archivo de su tarea en `tasks/`:

```markdown
## Reporte
- **Estado:** done | partial | blocked
- **Rama / commits:** dev/proto-01-framing · abc1234, def5678
- **Hecho:** …
- **Archivos principales:** …
- **Comandos:** fmt ✅ · clippy ✅ · clippy no-default ✅ · test ✅ (N tests) · doc ✅
- **Decisiones:** …
- **Pendiente / bloqueos:** …
- **Desviaciones de la spec o de .ai-context:** …
```
