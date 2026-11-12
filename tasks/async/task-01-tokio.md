# Tarea ASYNC-01: API async con tokio (feature `tokio`)

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-ASYNC-01` |
| **Estado** | ✅ terminada |
| **Rama** | `dev/async-01-tokio` |
| **Depende de** | CLIENT-01, SERIAL-01 |
| **Restricciones** | ADR-01, ADR-02, API-01, API-07, API-08, LINK-01…07, QA-03 · `public-api.md` §9 · `architecture.md` §3 (`tokio`, `tokio-serial`) |
| **Prioridad** | P2 |

## 1. Contexto

Apps async (servidores, Tauri con comandos `async`) pueden usar hoy la API bloqueante con `spawn_blocking`. Esta tarea ofrece una API nativa async **sin duplicar el protocolo**: reutiliza `protocol` y `message`, y reimplementa solo el enlace sobre `AsyncRead + AsyncWrite`.

## 2. Requerimientos

### RF-01 Feature y dependencias
Feature `tokio = ["dep:tokio"]` y `tokio-serial = ["tokio", "dep:tokio-serial", "serial"]` con las versiones/features de `architecture.md` §3 (agregar a `[workspace.dependencies]`). Sin la feature, nada cambia (API-08).

### RF-02 Enlace async
`transbank_pos::tokio::AsyncLink<S: AsyncRead + AsyncWrite + Unpin + Send>` con la misma semántica que `Link` (LINK-01…07), timeouts con `tokio::time::timeout` y `LinkConfig`. Para no duplicar la lógica de decisión, extrae (refactor mínimo y justificado de LINK-01) una máquina de estados sans-IO del intercambio (`ExchangeState`) usada por ambos enlaces, **o** justifica en el Reporte por qué no y cubre ambos con la misma batería de tests (macro o tabla compartida).

### RF-03 Cliente async
`AsyncPosIntegrado<S>` con los métodos de `PosIntegrado` en versión `async` (callbacks de estado como `FnMut` síncrono). Constructores `connect_tcp(addr)` y, con `tokio-serial`, `open_serial(path, baud)`.

### RF-04 Simulador
`transbank-pos-sim` expone `AsyncSimStream` (dúplex en memoria con `tokio::io::duplex`) o un servidor TCP async para tests — lo mínimo para QA-02 async.

## 3. Tests

- La batería de LINK-01 contra `AsyncLink` con `tokio::time::pause()` (tiempo simulado, QA-03).
- Cliente async contra el simulador: venta aprobada, rechazada, timeout ⇒ `last_sale`.
- `AsyncPosIntegrado<TcpStream>` es `Send` y su futuro de `sale` es `Send`.

## 4. Definición de terminado

- [ ] RF-01…RF-04.
- [ ] AGENTS.md §8 en verde (con `--all-features`).
- [ ] Reporte completo.

## Reporte
- **Estado:** done
- **Rama / commits:** `dev/async-01-tokio` · 0710306, 605e63f, 1e5cc44, a49276e, cf5d0a1, b207451
- **Hecho:**
  - Extraído el núcleo sans-IO del intercambio en `crates/transbank-pos/src/exchange.rs` (`AckWait`, `ResponseWait`, `ListTailWait`, `count_frame`, `is_list_terminator`, `function_code`, `unexpected`), sin E/S ni reloj, reutilizado de forma idéntica por `Link` y `AsyncLink`.
  - A1: Restituido el acuse obligatorio con ACK de tramas con LRC válido en el tail antes de devolver error mediante `ListTailStep::AckError(Error)` sin devolver `Result` en `on_event` de `ListTailWait`.
  - A2: Agregado helper privado `deadline(timeout)` en `AsyncLink` que acota la duración a 30 años con `timeout.min(...)` antes de sumar a `Instant::now()`, evitando pánicos con `Duration::MAX`.
  - A3: `AsyncPosIntegrado::open_serial` verifica `tokio::runtime::Handle::try_current()` y devuelve `Error::Transport` sin pánico si se llama fuera de un runtime Tokio.
  - A4: Reemplazados enlaces intra-doc a `tokio` en `crates/transbank-pos/src/lib.rs` por texto plano con backticks para pasar `cargo doc` sin features por defecto.
  - A5: Traducido el proptest `any_chunking_and_garbage` de `link_edge_cases.rs` a `tests/async_link.rs` con runtime local `start_paused(true)`. Batería completa de `tests/async_link.rs` cubre la totalidad de los 17 tests de `tests/link.rs` y los 15 de `tests/link_edge_cases.rs`, más trampas a–f y nuevos casos de tail (42 tests en total).
  - A6: Fortalecidos tests de drenado y cancelación: `trap_b` empuja trama válida `0810|00|OLD` y comprueba respuesta nueva `0810|00|NEW`; `trap_f` entrega respuesta tardía de venta y comprueba siguiente operación limpia.
  - A7: Todos los tests de `tests/client_async.rs` usan `start_paused = true` (salvo `connect_tcp_local_server` con E/S real); agregados escenarios `CorruptLrc(4)` ⇒ `Error::BadLrc` (1 `0200` recibido) y `CorruptLrc(2)`, `NakNext(2)` y `DropAcks(2)` aprobados con 1 venta en el simulador (13 tests en total).
  - A8: Documentados requisitos de runtime Tokio con driver de tiempo (`enable_time`) y E/S (`enable_all`) en la doc de `transbank_pos::tokio` y `AsyncLink`.
  - Agregadas las dependencias opcionales `tokio` (con features exactas `rt, time, io-util, net, sync`) y `tokio-serial` en `transbank-pos`, `tokio` (solo `io-util`) en `transbank-pos-sim`, y `macros`, `test-util` exclusivamente en `[dev-dependencies]`.
  - Implementado `transbank_pos::tokio::AsyncLink<S: AsyncRead + AsyncWrite + Unpin + Send>` con trampas a–f y enlace usable tras cancelación.
  - Implementado `transbank_pos::tokio::AsyncPosIntegrado<S>` con todos los métodos de `PosIntegrado` en versión async, constructores `connect_tcp` y `open_serial`.
  - Implementado `transbank_pos_sim::AsyncSimStream` dúplex en memoria implementando `AsyncRead + AsyncWrite` sobre `PosSimulator`.
- **Archivos principales:**
  - `crates/transbank-pos/src/exchange.rs`
  - `crates/transbank-pos/src/lib.rs`
  - `crates/transbank-pos/src/link.rs`
  - `crates/transbank-pos/src/tokio/mod.rs`
  - `crates/transbank-pos/src/tokio/link.rs`
  - `crates/transbank-pos/src/tokio/client.rs`
  - `crates/transbank-pos-sim/src/async_stream.rs`
  - `crates/transbank-pos/tests/async_link.rs`
  - `crates/transbank-pos/tests/client_async.rs`
  - `crates/transbank-pos/tests/link_edge_cases.rs`
- **Comandos:**
  - `fmt` ✅ (`flock /tmp/agents-locks/rust.lock cargo fmt --all --check`)
  - `clippy` ✅ (`flock /tmp/agents-locks/rust.lock cargo clippy --workspace --all-targets --all-features -- -D warnings`)
  - `clippy no-default` ✅ (`flock /tmp/agents-locks/rust.lock cargo clippy -p transbank-pos --no-default-features -- -D warnings`)
  - `clippy tokio` ✅ (`flock /tmp/agents-locks/rust.lock cargo clippy -p transbank-pos --no-default-features --features tokio -- -D warnings`)
  - `clippy sim` ✅ (`flock /tmp/agents-locks/rust.lock cargo clippy -p transbank-pos-sim --features tokio -- -D warnings`)
  - `test workspace` ✅ (`flock /tmp/agents-locks/rust.lock cargo test --workspace --all-features` -> 357 tests + 39 doctests en 1.7 s)
  - `test transbank-pos` ✅ (`flock /tmp/agents-locks/rust.lock cargo test -p transbank-pos` -> 189 tests + 31 doctests en 0.8 s)
  - `doc workspace` ✅ (`RUSTDOCFLAGS="-D warnings" flock /tmp/agents-locks/rust.lock cargo doc --workspace --no-deps --all-features`)
  - `doc no-deps default` ✅ (`RUSTDOCFLAGS="-D warnings" flock /tmp/agents-locks/rust.lock cargo doc -p transbank-pos --no-deps`)
- **Decisiones:**
  - `AsyncSimStream` implementa `AsyncRead + AsyncWrite` con cola `VecDeque<u8>` y registro de `Waker` en lugar de `tokio::io::duplex` o `spawn`, evitando background tasks y requisitos de multithread runtime (decisión del orquestador).
  - `AckWait`, `ResponseWait` y `ListTailWait` retornan pasos tipados (`AckStep`, `ResponseStep`, `ListTailStep`), permitiendo que el enlace síncrono y el asíncrono ejecuten exactamente las mismas transiciones de estado, mismas escrituras (ACK/NAK) y mismos errores.
  - Helper `Self::deadline(timeout)` acota la duración a 30 años con `timeout.min(...)` antes de sumar a `Instant::now()`, evitando pánicos por overflow temporal con `Duration::MAX`.
- **Pendiente / bloqueos:** Ninguno.
- **Desviaciones de la spec o de .ai-context:** Ninguna.
