# Arquitectura — transbank-SDK

## 1. Vista general

```
 Software del comercio (caja, kiosco, app Tauri…)
        │  API pública (public-api.md)
        ▼
┌──────────────────────────────── transbank-pos ────────────────────────────────┐
│  client      PosIntegrado<T> · PosAutoservicio<T>        (operaciones tipadas) │
│     │                                                                          │
│  link        Link<T>: envía trama → espera ACK (reintentos) → recibe          │
│     │        respuestas (ACK/NAK, 0900, detalle multi-trama, timeouts)         │
│     │                                                                          │
│  protocol    sans-IO, sin E/S ni reloj:                                        │
│              lrc · frame (encode) · decoder (FrameDecoder → LinkEvent)         │
│              message (requests → payload; payload → respuestas tipadas)        │
│              codes (ResponseCode, IntermediateStatus) · types (Amount, Ticket…)│
│     │                                                                          │
│  transport   trait Transport  ── MemoryTransport (tests)                       │
│                               ── TcpTransport   (simulador, puentes serie-red) │
│                               ── SerialTransport (feature "serial")            │
└──────────────────────────────────────────┬─────────────────────────────────────┘
                                           │ bytes
                     USB/RS-232 ───────────┴─────────── TCP 127.0.0.1
                          │                                  │
                     POS real                       transbank-pos-sim (PosSimulator)
```

- **Sans-IO primero (ADR-01):** todo lo que depende solo de bytes (LRC, tramas, decodificador, construir y parsear mensajes) vive en `protocol` sin E/S ni tiempo. Se prueba exhaustivamente con vectores y `proptest`, y el simulador lo reutiliza.
- **El enlace es genérico sobre `Transport`** y es el único lugar con timeouts y reintentos.
- **El cliente es delgado:** valida argumentos, arma el request, llama al enlace y parsea.

## 2. Workspace

```
transbank-SDK/
├── Cargo.toml                 # [workspace] resolver = "3", [workspace.package], [workspace.lints], [workspace.dependencies]
├── Cargo.lock                 # versionado (hay binarios)
├── clippy.toml                # allow-unwrap-in-tests, allow-expect-in-tests, allow-panic-in-tests, allow-indexing-slicing-in-tests
├── rustfmt.toml               # edition = "2024" (resto por defecto)
├── .github/workflows/ci.yml   # manual + push a main/develop (INFRA-01)
└── crates/
    ├── transbank-pos/
    │   ├── Cargo.toml         # features: default = ["serial"], serial, serde, tracing
    │   ├── src/
    │   │   ├── lib.rs         # docs de crate, re-exports, #![forbid(unsafe_code)]
    │   │   ├── error.rs       # Error, Result
    │   │   ├── protocol/      # mod.rs, lrc.rs, frame.rs, decoder.rs, fields.rs (split/trim/parse), types.rs
    │   │   ├── message/       # mod.rs, integrado.rs, autoservicio.rs, responses.rs
    │   │   ├── codes.rs       # ResponseCode, IntermediateStatus
    │   │   ├── link.rs        # Link<T>, LinkConfig
    │   │   ├── client/        # mod.rs, integrado.rs, autoservicio.rs
    │   │   └── transport/     # mod.rs (trait), memory.rs, tcp.rs, serial.rs (cfg feature)
    │   ├── tests/             # integración: cliente ↔ simulador en memoria; fixtures/ (tramas reales de los SDKs)
    │   └── examples/          # poll.rs, sale.rs, detail.rs
    ├── transbank-pos-sim/
    │   ├── src/lib.rs         # PosSimulator (sans-IO), Scenario, SimTransport (memoria)
    │   └── src/main.rs        # bin tbk-pos-sim: servidor TCP
    └── transbank-pos-cli/
        └── src/main.rs        # bin tbk-pos
```

`transbank-pos-sim` depende de `transbank-pos` (para reutilizar `protocol`). `transbank-pos` usa `transbank-pos-sim` solo como **dev-dependency** (tests de integración). La CLI depende de ambos (`--sim` en memoria).

## 3. Stack (cerrado)

| Qué | Versión / valor |
|:--|:--|
| Rust | estable, **MSRV 1.85**, edición **2024** (`rust-version = "1.85"` en `[workspace.package]`) |
| Licencia | `MIT OR Apache-2.0` |
| `unsafe` | `forbid` en todo el workspace |

### Dependencias permitidas (no hay otras)

| Crate | Versión | Dónde | Features | Para qué |
|:--|:--|:--|:--|:--|
| `thiserror` | 2 | transbank-pos | — | `Error` |
| `serialport` | 4 | transbank-pos (optional, feature `serial`) | por defecto del crate | Puerto serie y listado de puertos |
| `serde` | 1 | transbank-pos (optional, feature `serde`) | `derive` | Serializar respuestas (p. ej. para Tauri) |
| `tracing` | 0.1 | transbank-pos (optional, feature `tracing`) | — | Trazas de enlace (`security-model.md` §3) |
| `clap` | 4 | transbank-pos-cli, transbank-pos-sim (bin) | `derive` | Argumentos |
| `serde_json` | 1 | transbank-pos-cli | — | Salida `--json` |
| `tracing-subscriber` | 0.3 | transbank-pos-cli | `fmt`, `env-filter` | `--trace-file` / `RUST_LOG` (trazas redactadas de la librería) |
| `proptest` | 1 | dev-dependency | — | Propiedades del decodificador y del parseo |
| `tokio` | 1 | **solo desde ASYNC-01**, optional | `rt`, `time`, `io-util`, `net`, `sync` | API async |
| `tokio-serial` | 5 | **solo desde ASYNC-01**, optional | — | Puerto serie async |

Sin `anyhow`, sin `chrono`/`time` (fechas del POS con tipos propios), sin `log`, sin `bytes`, sin `once_cell`/`lazy_static`.

## 4. Decisiones (ADR)

| ADR | Decisión | Por qué |
|:--|:--|:--|
| ADR-01 | Núcleo sans-IO (`protocol`) separado del enlace y del transporte | Testeable byte a byte; el simulador y una futura API async lo reutilizan sin duplicar |
| ADR-02 | API **síncrona/bloqueante** primero; async (tokio) como feature opcional en ASYNC-01 | El POS es lento (segundos) y de una operación a la vez; bloqueante es más simple y sirve a Tauri vía `spawn_blocking`. No imponer runtime |
| ADR-03 | `Err` solo para fallas de comunicación/protocolo; los rechazos son respuestas `Ok` | Una tarjeta rechazada es un resultado de negocio (`domain.md` §6) |
| ADR-04 | Sin reintento automático de ventas/anulaciones tras el ACK | Evita doble cobro; la app decide con `last_sale()` |
| ADR-05 | Decodificador por máquina de estados (STX/ETX/LRC), no por "pausa entre bytes" como el SDK Node | Robusto ante trozos arbitrarios y ante LRC = byte de control |
| ADR-06 | Texto recibido como ISO-8859-1, enviado solo ASCII | Nunca falla al decodificar; los vouchers traen tildes |
| ADR-07 | Fechas del POS como tipos propios sin zona (`PosDate`, `PosTime`) | El POS no informa zona; evitar dependencias de fechas |
| ADR-08 | Transporte como trait con lectura con timeout (`read_timeout`) | Permite memoria con tiempo simulado, TCP y serie con el mismo enlace |
| ADR-09 | Tiempo inyectable: el enlace usa un `Clock` (trait) — `SystemClock` en producción, reloj manual en tests | Tests de timeouts deterministas y rápidos (QA-03) |
| ADR-10 | Simulador como crate propio, con lib sans-IO + bin TCP | Los usuarios del SDK también lo usan para sus propios tests |
| ADR-11 | Dual license MIT/Apache-2.0 + `NOTICE` con BSD-3 de Transbank | Convención del ecosistema Rust; respeta la licencia de los SDKs de origen |
