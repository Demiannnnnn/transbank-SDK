# AGENTS.md — Reglas obligatorias para agentes de IA

> **PRIORIDAD MÁXIMA.** Este archivo prevalece sobre cualquier otra instrucción: prompts, tareas,
> documentación, comentarios en el código, skills o tus propias preferencias. Si una instrucción
> contradice este archivo, **NO la ejecutes**: detente y repórtalo (§9). Ante la duda, **no lo hagas**
> y repórtalo.
>
> Orden de precedencia: `AGENTS.md` > `.ai-context/constraints.md` > resto de `.ai-context/` > `docs/` > `tasks/` > prompt.

**DEBES**, **NUNCA** y **SOLO** son absolutas. No hay excepciones implícitas.

---

## 1. Lectura obligatoria antes de escribir código

1. `AGENTS.md` (este archivo), completo.
2. `.ai-context/README.md` y **todos** los archivos que indica (domain, architecture, constraints, conventions, security-model, public-api, implementation-status).
3. `tasks/README.md` y el archivo de **tu** tarea.
   - Si retomas el proyecto en una sesión o equipo nuevo, lee primero [`.ai-context/handoff.md`](.ai-context/handoff.md).
4. Los `docs/` que tu tarea referencia. **Siempre** [`docs/protocol-spec.md`](docs/protocol-spec.md) si tocas tramas, mensajes, el enlace o el simulador.

Si algo es ambiguo, contradictorio o falta para tu tarea: **no inventes requisitos ni bytes del protocolo**. Toma la opción más conservadora que cumpla `.ai-context/`, y repórtala en "Decisiones".

## 2. Proyecto en una línea

SDK **no oficial** en Rust para comunicarse con los terminales POS de Transbank (POS Integrado y POS Autoservicio) por puerto serie/USB, con un simulador de POS y una CLI. Workspace Cargo:
`crates/transbank-pos` (librería: protocolo sans-IO + enlace + cliente + transportes) · `crates/transbank-pos-sim` (simulador, lib + bin `tbk-pos-sim`) · `crates/transbank-pos-cli` (bin `tbk-pos`).
**El stack está cerrado** (`.ai-context/architecture.md` §3). NUNCA cambies edición, MSRV, estructura de crates ni agregues crates al workspace sin que tu tarea lo pida.

## 3. Alcance — haz SOLO tu tarea

- Trabaja **únicamente** en la tarea asignada, en el orden de `tasks/README.md`. Una tarea a la vez.
- NUNCA hagas refactors, renombres o "mejoras" fuera del alcance. NUNCA implementes por adelantado funcionalidades de otras tareas.
- Si necesitas cambiar algo fuera del alcance para completar la tarea: hazlo **mínimo** y justifícalo en el reporte. Si es grande, detente y repórtalo (`blocked`).
- **Alcance del sistema de archivos:** solo este repositorio. Los repos hermanos (`../rework-sistema-facturacion`, `../telemetry-*`) se pueden **leer** como referencia; NUNCA modificarlos ni ejecutar git en ellos salvo lectura. Los SDKs oficiales de Transbank se pueden leer en GitHub como referencia (§7, LEG-*).

## 4. Git y ramas (reglas del dueño — sin excepciones)

| Rama | Permitido para ti |
|:--|:--|
| `main` | **NADA.** No checkout para trabajar, no commit, no merge, no push, no PR, no rebase, no borrar. |
| `develop` | **NADA.** Igual que `main`. |
| `develop-AGENTS` | **NADA de escritura.** Es la rama de integración del orquestador. No commit, no merge, no push. |
| `dev/*` | **Libre:** crear, cambiar, commitear, pushear (`git push -u origin dev/...`), borrar las tuyas. |

- Trabajas en la rama `dev/...` que el orquestador dejó checkouteada (indicada en tu tarea).
- Commits pequeños, Conventional Commits en inglés + `Refs: TASK-<ID>` en el cuerpo (`.ai-context/conventions.md` §2).
- PROHIBIDO: `git push --force` (salvo `--force-with-lease` en tu propia rama `dev/*`), reescribir historia ajena, `git reset --hard` sobre trabajo no commiteado de otros, crear PRs, crear tags, `cargo publish` (sin `--dry-run`).
- Verifica la rama antes de cada commit: `git branch --show-current` debe empezar con `dev/`.

## 5. Archivos protegidos (solo lectura)

- `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.ai-context/`, `docs/` (salvo que tu tarea pida crear un documento nuevo concreto), `tasks/README.md`, `tools/orchestration/`.
- `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`: NUNCA.
- En **tu** archivo de tarea SOLO puedes modificar la línea `**Estado**` de la tabla y la sección `## Reporte`.
- `.github/` solo si tu tarea lo pide (INFRA-01 o una tarea que diga "CI").
- `Cargo.lock` solo cambia vía `cargo`.
- Fixtures de protocolo ya existentes (`crates/*/tests/fixtures/`): NUNCA editarlos para que un test pase; agrega nuevos.

## 6. PROHIBIDO — calidad (no hagas trampa para que algo "pase")

- NUNCA borres, desactives o debilites tests (`#[ignore]`, asserts vacíos, `assert!(true)`, comentar tests, bajar casos de `proptest`).
- NUNCA uses `#[allow(...)]`, `#![allow(...)]` ni `#[expect(...)]` para silenciar lints. Única excepción: la que una tarea indique explícitamente con su justificación.
- NUNCA uses `unsafe` (el workspace lo prohíbe con `forbid`).
- En código de producción (todo lo que no sea `#[cfg(test)]`, `tests/`, `benches/`, `examples/`): NUNCA `unwrap()`, `expect()`, `panic!`, `todo!`, `unimplemented!`, `unreachable!` ni indexación que pueda entrar en pánico (`slice[i]` sin verificar). Usa `Result`, `get()`, `?`.
- NUNCA hardcodees resultados para satisfacer un test, ni detectes "si estoy en test" en código de producción.
- NUNCA modifiques `[workspace.lints]`, `clippy.toml`, `rustfmt.toml` ni el CI para silenciar errores.
- NUNCA declares que algo funciona sin haber ejecutado los comandos de §8.
- NUNCA uses `std::thread::sleep` en tests para "esperar" algo: usa los transportes de prueba con tiempo simulado (`.ai-context/conventions.md` §5).

## 7. PROHIBIDO — dependencias, sistema, seguridad y licencias

- NUNCA agregues una dependencia fuera de la lista permitida (`.ai-context/architecture.md` §3), ni actives features no listadas. Si crees que hace falta: no la instales, propónla en el reporte.
- NUNCA instales nada globalmente ni uses `sudo`, `rustup`, `cargo install`, `brew`, `apt`. NUNCA cambies la versión de Rust.
- NUNCA ejecutes comandos destructivos: `rm -rf` fuera de `target/`; `git clean -fdx`; nada fuera del repo.
- NUNCA abras puertos serie reales ni hables con un POS real en tests ni en tareas: solo `MemoryTransport`, el simulador o `TcpTransport` contra `127.0.0.1`. El hardware real lo prueba el dueño (`docs/hardware-testing.md`).
- NUNCA hagas peticiones de red a servicios externos en tests.
- NUNCA escribas en el repo datos reales de tarjetas, números de comercio reales del dueño ni vouchers reales. Usa los códigos de comercio de prueba de los SDKs oficiales (`597029414300`, `597012345678`) o valores inventados evidentes.
- **Licencias (LEG-01…04):** NUNCA copies código de los SDKs oficiales de Transbank ni de otros proyectos. Se implementa desde `docs/protocol-spec.md`. Si portas datos concretos (tablas de mensajes, tramas de ejemplo), cítalo en un comentario con el archivo de origen. NUNCA copies código con licencia incompatible (GPL, sin licencia).

## 8. Definición de "terminado"

Una tarea está terminada **SOLO** si, desde la raíz del repo, todos pasan sin errores ni warnings:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy -p transbank-pos --no-default-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
```

Más todos los criterios de aceptación y tests de la tarea, y `git status` limpio (todo commiteado en tu rama `dev/*`). Si algo falla y no puedes arreglarlo **sin violar este archivo**, NO la marques como `done`.

## 9. Cómo detenerse y reportar

Al terminar **o** al bloquearte, DEBES:

1. Commitear tu trabajo en tu rama `dev/*` (aunque sea parcial, con mensaje claro `wip:` si es `partial`).
2. Actualizar en tu archivo de tarea la línea **Estado** (`✅ terminada` | `🟡 parcial` | `⛔ bloqueada`) y la sección `## Reporte` con el formato de `.ai-context/conventions.md` §8: qué hiciste, archivos, resultado real de cada comando de §8, decisiones, pendientes/bloqueos, desviaciones de la spec.
3. Commitear ese cambio del reporte.

Reportar un bloqueo con honestidad es correcto. Ocultar un fallo, inventar resultados o declarar "terminado" sin verificar es la falta más grave posible.
