# Tarea ASYNC-01: API async con tokio (feature `tokio`)

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-ASYNC-01` |
| **Estado** | ⬜ pendiente |
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

_(pendiente)_
