# Tarea INFRA-01: Workspace Cargo, lints, licencias por crate y CI manual

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-INFRA-01` |
| **Estado** | ⬜ pendiente |
| **Rama** | `dev/infra-01-workspace` |
| **Depende de** | — |
| **Restricciones** | SAFE-01, LEG-01, LEG-03, LEG-04, API-08, QA-03 · `.ai-context/architecture.md` §2–§3, `conventions.md` §3–§4 |
| **Prioridad** | P0 |

## 1. Contexto

El repo solo tiene documentación. Esta tarea crea el esqueleto del workspace de tres crates con lints estrictos, para que todas las tareas siguientes partan de un `cargo test` verde. **No** implementa protocolo ni lógica: solo estructura, metadatos y placeholders documentados.

## 2. Requerimientos

### RF-01 Workspace raíz
- `Cargo.toml` con `[workspace]` `resolver = "3"`, `members = ["crates/*"]`.
- `[workspace.package]`: `version = "0.1.0"`, `edition = "2024"`, `rust-version = "1.85"`, `license = "MIT OR Apache-2.0"`, `repository = "https://github.com/Demiannnnnn/transbank-SDK"`, `authors = ["Demian"]`.
- `[workspace.dependencies]` con **exactamente** las dependencias de `architecture.md` §3 (salvo `tokio`/`tokio-serial`, que agrega ASYNC-01) con sus versiones mayores y features; los crates las usan con `workspace = true`.
- `[workspace.lints]` exactamente como `conventions.md` §3.
- `clippy.toml` con los cuatro `allow-*-in-tests`. `rustfmt.toml` con `edition = "2024"`.
- `.gitignore`: `/target`. `Cargo.lock` versionado.

### RF-02 Crates
- `crates/transbank-pos` (lib): features `default = ["serial"]`, `serial = ["dep:serialport"]`, `serde = ["dep:serde"]`, `tracing = ["dep:tracing"]`. `lib.rs` con `#![forbid(unsafe_code)]`, documentación de crate (qué es, aviso **"Unofficial — not affiliated with Transbank"** (LEG-03), enlace a la spec) y los módulos vacíos de `architecture.md` §2 declarados con su `//!` (cada uno documentado, sin ítems públicos aún). `error.rs` puede quedar para PROTO-01.
- `crates/transbank-pos-sim` (lib + `[[bin]] name = "tbk-pos-sim"`): depende de `transbank-pos` (`path`, `version = "0.1.0"`, `default-features = false`). `main.rs` mínimo que imprime la ayuda de `clap` con el aviso no oficial.
- `crates/transbank-pos-cli` (`[[bin]] name = "tbk-pos"`): depende de `transbank-pos` y `transbank-pos-sim`. `main.rs` mínimo con `clap` (`--version`, `--help` con aviso no oficial).
- Cada crate: `description` que empieza por "Unofficial", `readme`, `keywords`, `categories`, `include = ["src/**", "Cargo.toml", "README.md", "../../LICENSE-*", "../../NOTICE"]` (si `include` con `../..` no funciona en `cargo package`, copia nada: deja `license` y repórtalo; DOCS-01 lo resuelve), `[lints] workspace = true`.
- README corto por crate (inglés) con aviso unofficial y enlace al README raíz.

### RF-03 CI (`.github/workflows/ci.yml`)
- Disparadores: `workflow_dispatch` y `push` a `main` y `develop` **solamente** (decisión del dueño: usar poco GitHub Actions).
- Matriz `ubuntu-latest`, `macos-latest`, `windows-latest`; toolchain estable con `dtolnay/rust-toolchain@stable` (componentes clippy, rustfmt) y un job extra con `1.85` que solo compila (`cargo build --workspace --all-features`).
- En Ubuntu instalar `libudev-dev` (serialport).
- Pasos: los cinco comandos de AGENTS.md §8. Cache con `Swatinem/rust-cache@v2`.

## 3. Reglas

- Ninguna dependencia fuera de la lista. Ningún `#[allow]` (el único allow es `module_name_repetitions`, en el workspace).
- No escribir lógica del protocolo (se hace en PROTO-01/02).

## 4. Tests

- Un test de humo por crate (`tests/smoke.rs` o unit) que verifique algo real: en `transbank-pos`, que `env!("CARGO_PKG_LICENSE") == "MIT OR Apache-2.0"`; en la CLI, que `tbk-pos --help` (vía `env!("CARGO_BIN_EXE_tbk-pos")`) sale con código 0 y contiene "Unofficial" o "no oficial".
- `cargo clippy -p transbank-pos --no-default-features -- -D warnings` pasa (API-08).

## 5. Definición de terminado

- [ ] RF-01…RF-03.
- [ ] AGENTS.md §8 en verde (los cinco comandos).
- [ ] `cargo package -p transbank-pos --allow-dirty --no-verify --list` incluye las licencias o el Reporte explica por qué no.
- [ ] Reporte completo.

## Reporte

_(pendiente)_
