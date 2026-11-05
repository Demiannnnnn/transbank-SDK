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
- **Rama / commits:** `dev/async-01-tokio` · 0710306, 605e63f, 1e5cc44, a49276e, cf5d0a1
- **Hecho:**
  - Extraído el núcleo sans-IO del intercambio en `crates/transbank-pos/src/exchange.rs` (`AckWait`, `ResponseWait`, `ListTailWait`, `count_frame`, `is_list_terminator`, `function_code`, `unexpected`), sin E/S ni reloj, reutilizado de forma idéntica por `Link` y `AsyncLink`.
  - Agregadas las dependencias opcionales `tokio` (con features exactas `rt, time, io-util, net, sync`) y `tokio-serial` en `transbank-pos`, `tokio` (solo `io-util`) en `transbank-pos-sim`, y `macros`, `test-util` exclusivamente en `[dev-dependencies]`.
  - Implementado `transbank_pos::tokio::AsyncLink<S: AsyncRead + AsyncWrite + Unpin + Send>` con las trampas a–f (EOF como `UnexpectedEof`, drenado de entrada vieja no bloqueante con `poll_fn`, `flush()` tras cada escritura, plazos fijos con `timeout_at` sin reinicio por `0900`, LINK-03 sin reenvío tras ACK, cancelación documentada).
  - Implementado `transbank_pos::tokio::AsyncPosIntegrado<S>` con todos los métodos de `PosIntegrado` en versión async, constructores `connect_tcp` (con `set_nodelay` y timeout de 5 s) y `open_serial` (8N1 sin control de flujo).
  - Implementado `transbank_pos_sim::AsyncSimStream` dúplex en memoria implementando directamente `AsyncRead + AsyncWrite` sobre `PosSimulator` sin `spawn` ni `duplex`.
  - Batería completa de tests: traducción 1-a-1 de los 17 tests de `tests/link.rs` y los 14 de `tests/link_edge_cases.rs` más 6 tests específicos de las trampas a–f en `tests/async_link.rs` (37 tests en total con `start_paused = true`); 8 tests exhaustivos de cliente async contra `AsyncSimStream` y TCP local en `tests/client_async.rs`.
- **Archivos principales:**
  - `crates/transbank-pos/src/exchange.rs`
  - `crates/transbank-pos/src/link.rs`
  - `crates/transbank-pos/src/tokio/mod.rs`
  - `crates/transbank-pos/src/tokio/link.rs`
  - `crates/transbank-pos/src/tokio/client.rs`
  - `crates/transbank-pos-sim/src/async_stream.rs`
  - `crates/transbank-pos/tests/async_link.rs`
  - `crates/transbank-pos/tests/client_async.rs`
- **Comandos:**
  - `fmt` ✅ (`cargo fmt --all --check`)
  - `clippy` ✅ (`cargo clippy --workspace --all-targets --all-features -- -D warnings`)
  - `clippy no-default` ✅ (`cargo clippy -p transbank-pos --no-default-features -- -D warnings`)
  - `clippy tokio` ✅ (`cargo clippy -p transbank-pos --no-default-features --features tokio -- -D warnings`)
  - `test` ✅ (311 tests + 39 doctests workspace en 1.5 s; 198 tests + 31 doctests transbank-pos)
  - `doc` ✅ (`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features`)
- **Decisiones:**
  - `AsyncSimStream` implementa `AsyncRead + AsyncWrite` con cola `VecDeque<u8>` y registro de `Waker` en lugar de `tokio::io::duplex` o `spawn`, evitando background tasks y requisitos de multithread runtime (decisión del orquestador).
  - `AckWait` y `ResponseWait` retornan pasos tipados (`AckStep`, `ResponseStep`, `ListTailStep`), permitiendo que el enlace síncrono y el asíncrono ejecuten exactamente las mismas transiciones de estado, mismas escrituras (ACK/NAK) y mismos errores.
- **Pendiente / bloqueos:** Ninguno.
- **Desviaciones de la spec o de .ai-context:**
  - `AsyncSimStream` implementado directamente sobre `PosSimulator` sin `tokio::io::duplex` (anotado según instrucción de tarea RF-04).
