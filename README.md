# transbank-SDK

SDK **no oficial** en Rust para comunicarse con los terminales **POS de Transbank** (Chile) conectados por USB o puerto serie: **POS Integrado** (Verifone VX520/VX520c, Ingenico Desk 3500) y **POS Autoservicio**. Implementa el protocolo serial (tramas STX/ETX/LRC, ACK/NAK, reintentos) e incluye un **simulador de POS** y una **CLI** para probar sin hardware.

> ⚠️ **Proyecto no oficial.** No está afiliado, respaldado ni soportado por Transbank S.A. El protocolo se derivó de los [SDKs oficiales de Transbank](https://github.com/TransbankDevelopers) (BSD-3-Clause; ver [`NOTICE`](NOTICE)). Úsalo bajo tu responsabilidad y valida siempre con tu terminal antes de cobrar de verdad.

> Estado: **implementado, sin validar con un POS real.** Protocolo, enlace, cliente `PosIntegrado`, transportes, simulador y CLI listos y probados contra el simulador; falta la validación con hardware (HW-01) y POS Autoservicio. Detalle en [`.ai-context/implementation-status.md`](.ai-context/implementation-status.md) y [`tasks/README.md`](tasks/README.md). Aún no publicado en crates.io.

## Qué ofrece (v0.1)

- **`transbank-pos`** — librería:
  - Protocolo *sans-IO*: codificador/decodificador de tramas, LRC, requests y parseo tipado de respuestas.
  - Capa de enlace con ACK/NAK, 3 reintentos y timeouts configurables.
  - Cliente `PosIntegrado`: `poll`, `load_keys`, `sale`, `multicode_sale`, `last_sale`, `refund`, `totals`, `sales_detail`, `close`, `set_normal_mode`, con mensajes intermedios ("Esperando leer tarjeta", "Ingrese clave"…).
  - Transportes: puerto serie (`serialport`, feature `serial`), TCP y memoria (tests).
  - Montos en CLP enteros, tipos sin pánicos, `#![forbid(unsafe_code)]`.
- **`transbank-pos-sim`** — POS Integrado simulado (aprobaciones, rechazos, timeouts, LRC corrupto…) en memoria o como servidor TCP (`tbk-pos-sim`).
- **`transbank-pos-cli`** — CLI `tbk-pos` para operar un POS real o el simulador desde la terminal.

## Ejemplo

```rust
use transbank_pos::{PosIntegrado, SaleRequest, serial::SerialTransport};

let transport = SerialTransport::open("/dev/ttyACM0", 115_200)?;
let mut pos = PosIntegrado::new(transport);
pos.poll()?;

let sale = pos.sale_with_status(&SaleRequest::new(12_000, "ABC123")?, |status| {
    println!("POS: {}", status.message());
})?;

if sale.is_approved() {
    println!("Aprobada: autorización {:?}", sale.authorization_code);
} else {
    println!("No aprobada: {}", sale.response_code.message());
}
```

La API normativa está en [`.ai-context/public-api.md`](.ai-context/public-api.md).

## Requisitos

- Rust estable **≥ 1.85** (edición 2024), con `clippy` y `rustfmt`.
- Linux: `libudev-dev` / `systemd-libs` para listar puertos USB (`serialport`), y permiso sobre el puerto (grupo `dialout` o `uucp`).
- Un POS de Transbank **en modo integrado** (lo activa Transbank) para el uso real. Para desarrollar basta el simulador.

## Arranque rápido

```bash
cargo build --workspace
cargo test --workspace --all-features

# Simulador en una terminal…
cargo run -p transbank-pos-sim -- --listen 127.0.0.1:7777
# …y la CLI contra él en otra
cargo run -p transbank-pos-cli -- --tcp 127.0.0.1:7777 sale 12000 ABC123

# Con un POS real
cargo run -p transbank-pos-cli -- ports
cargo run -p transbank-pos-cli -- --port /dev/ttyACM0 poll
```

## Calidad (definición de terminado)

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
```

## Estructura

```
transbank-SDK/
├── AGENTS.md  CLAUDE.md  README.md  NOTICE  LICENSE-MIT  LICENSE-APACHE
├── .ai-context/          # contexto normativo (dominio, arquitectura, restricciones, API)
├── docs/                 # protocolo, simulador, pruebas con hardware, publicación
├── tasks/                # plan maestro y tareas
├── tools/orchestration/  # scripts del orquestador (Claude + agy); no son parte del SDK
└── crates/
    ├── transbank-pos/        # librería
    ├── transbank-pos-sim/    # simulador (lib + bin tbk-pos-sim)
    └── transbank-pos-cli/    # bin tbk-pos
```

## Documentación

| Documento | Enlace |
|:--|:--|
| Reglas para agentes de IA | [`AGENTS.md`](AGENTS.md) |
| Contexto (dominio, arquitectura, restricciones, API pública) | [`.ai-context/`](.ai-context/README.md) |
| Especificación del protocolo (normativa) | [`docs/protocol-spec.md`](docs/protocol-spec.md) |
| Simulador | [`docs/simulator-spec.md`](docs/simulator-spec.md) |
| Pruebas con un POS real | [`docs/hardware-testing.md`](docs/hardware-testing.md) |
| Publicación en crates.io | [`docs/release.md`](docs/release.md) |
| Plan de tareas | [`tasks/README.md`](tasks/README.md) |

## Ramas

- `main` y `develop`: solo las integra el dueño del repositorio.
- `develop-AGENTS`: rama de integración de los agentes de IA, gestionada por el orquestador (lo más reciente está aquí).
- `dev/*`: ramas de trabajo de cada tarea.

## Contribuir

Issues y PRs bienvenidos (contra `develop`). Especialmente útil: **trazas reales** de tu POS (con datos sensibles borrados) para cerrar las preguntas abiertas de [`docs/protocol-spec.md` §10](docs/protocol-spec.md#10-preguntas-abiertas-se-cierran-con-un-pos-real--hw-01).

## Licencia

A tu elección, [Apache-2.0](LICENSE-APACHE) o [MIT](LICENSE-MIT). Las partes derivadas de los SDKs oficiales de Transbank conservan su aviso BSD-3-Clause ([`NOTICE`](NOTICE)).
