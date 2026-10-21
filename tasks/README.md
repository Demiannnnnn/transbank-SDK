# Plan maestro de tareas — transbank-SDK

Mismo formato que `rework-sistema-facturacion` y `telemetry-*`: cada tarea tiene contexto, requerimientos (`RF-*`), reglas, tests, DoD y una sección `## Reporte` que completa el agente.

> Antes de tomar una tarea lee `AGENTS.md`, todo `.ai-context/` y [`../.ai-context/implementation-status.md`](../.ai-context/implementation-status.md). Si el código y una tarea discrepan, `.ai-context` y `docs/protocol-spec.md` mandan sobre el pseudocódigo de la tarea; repórtalo.

## 1. Principios

1. **Bytes exactos de la spec** (PROTO-01): cada trama tiene un test byte a byte.
2. **Sans-IO primero** (ADR-01): protocolo sin E/S ni reloj; enlace genérico sobre `Transport`.
3. **Nunca doble cobro** (ADR-04, LINK-03) y **rechazo ≠ error** (ADR-03).
4. **Sin pánicos ni `unsafe`** (AGENTS.md §6, SAFE-01).
5. **Tests deterministas** con `ManualClock` y el simulador (QA-03); nada de hardware real.
6. **Cada tarea deja el workspace verde** (AGENTS.md §8).

## 2. Estructura

```
tasks/
├── README.md
├── infra/   task-01-workspace-and-ci
├── proto/   task-01-framing · task-02-integrado-messages
├── link/    task-01-transport-and-link
├── client/  task-01-pos-integrado
├── sim/     task-01-simulator
├── serial/  task-01-serial-and-tcp
├── cli/     task-01-cli
├── auto/    task-01-autoservicio
├── async/   task-01-tokio
├── docs/    task-01-docs-and-release-prep
└── hw/      task-01-hardware-validation  (manual, dueño)
```

## 3. Orden de ejecución

| # | ID | Tarea | Depende de | Prioridad | Estado |
|:--|:--|:--|:--|:--|:--|
| 1 | [INFRA-01](infra/task-01-workspace-and-ci.md) | Workspace Cargo, lints, licencias por crate, CI manual | — | P0 | ✅ |
| 2 | [PROTO-01](proto/task-01-framing.md) | LRC, codificador y decodificador de tramas | INFRA-01 | P0 | ✅ |
| 3 | [PROTO-02](proto/task-02-integrado-messages.md) | Tipos, códigos, requests y respuestas del POS Integrado | PROTO-01 | P0 | ⬜ |
| 4 | [LINK-01](link/task-01-transport-and-link.md) | `Transport`, `Clock`, `MemoryTransport`, enlace ACK/NAK/timeouts | PROTO-02 | P0 | ⬜ |
| 5 | [SIM-01](sim/task-01-simulator.md) | Simulador sans-IO, escenarios, `SimTransport`, bin TCP | PROTO-02, LINK-01 | P0 | ⬜ |
| 6 | [CLIENT-01](client/task-01-pos-integrado.md) | Cliente `PosIntegrado` + tests contra el simulador | PROTO-02, LINK-01, SIM-01 | P0 | ⬜ |
| 7 | [SERIAL-01](serial/task-01-serial-and-tcp.md) | `SerialTransport`, `list_ports`, `autoconnect`, `TcpTransport` | LINK-01, CLIENT-01 | P1 | ⬜ |
| 8 | [CLI-01](cli/task-01-cli.md) | CLI `tbk-pos` | CLIENT-01, SERIAL-01 | P1 | ⬜ |
| 9 | [DOCS-01](docs/task-01-docs-and-release-prep.md) | Rustdoc, ejemplos, READMEs de crates, CHANGELOG, `publish --dry-run` | CLI-01 | P1 | ⬜ |
| 10 | [HW-01](hw/task-01-hardware-validation.md) | Validación con POS real (**dueño**) | CLI-01 | P0 para publicar | ⬜ |
| 11 | [AUTO-01](auto/task-01-autoservicio.md) | POS Autoservicio (cliente + simulador + CLI) | CLIENT-01, SIM-01 | P2 | ⬜ |
| 12 | [ASYNC-01](async/task-01-tokio.md) | API async con tokio | CLIENT-01, SERIAL-01 | P2 | ⬜ |

**Paralelismo posible:** SERIAL-01 ∥ AUTO-01 (tras CLIENT-01); AUTO-01 ∥ ASYNC-01. El núcleo (INFRA → PROTO → LINK → SIM → CLIENT) es secuencial.

### Tareas futuras (no iniciar sin decisión del dueño)

| ID | Tarea | Notas |
|:--|:--|:--|
| REL-01 | Publicar en crates.io | Solo con autorización del dueño tras HW-01 (`docs/release.md`) |
| INT-01 | Integrar en `rework-sistema-facturacion` (app Tauri: cobro con POS en la venta) | Se especifica en ese repo; consume la versión publicada o `git` |
| FUT-01 | Bluetooth / POS Android de Transbank | Investigar si hay protocolo documentado |

Leyenda: ⬜ pendiente · 🔄 en curso · 🟡 parcial · ✅ terminada (validada por el orquestador) · ⛔ bloqueada.

## 4. Grafo de dependencias

```
INFRA-01 → PROTO-01 → PROTO-02 → LINK-01 → SIM-01 → CLIENT-01 ─┬→ SERIAL-01 → CLI-01 ─┬→ DOCS-01
                                                                ├→ AUTO-01             └→ HW-01 (dueño) → REL-01
                                                                └→ ASYNC-01 (tras SERIAL-01)
```

## 5. Plantilla de tarea

```markdown
# Tarea <ID>: <título>

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-<ID>` |
| **Estado** | ⬜ pendiente |
| **Rama** | `dev/<id>-<tema>` |
| **Depende de** | … |
| **Restricciones** | IDs de constraints.md · docs |
| **Prioridad** | P0/P1/P2 |

## 1. Contexto
## 2. Requerimientos (RF-01…)
## 3. Reglas
## 4. Tests
## 5. Definición de terminado
## Reporte
```
