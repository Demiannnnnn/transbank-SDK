# AI Context — transbank-SDK

Lee este directorio antes de tocar código. Es la fuente de verdad de **lo que el SDK debe ser**; el código es la fuente de verdad de **lo que existe hoy**. Si difieren, `implementation-status.md` dice qué brechas se conocen; cualquier otra diferencia es un bug en el código o en estos documentos, y debes reportarla.

Estos archivos son **de solo lectura para los agentes** (AGENTS.md §5). Si algo está mal o falta, dilo en el reporte de la tarea; el orquestador los actualiza.

## Orden de lectura

| # | Archivo | Léelo cuando |
|:--|:--|:--|
| 1 | [`domain.md`](domain.md) | Siempre. Lenguaje ubicuo, actores, operaciones del POS, montos, alcance (qué NO es). |
| 2 | [`architecture.md`](architecture.md) | Siempre. Workspace, capas, stack y versiones, dependencias permitidas, decisiones (ADR). |
| 3 | [`constraints.md`](constraints.md) | Siempre. Reglas duras con ID (`PROTO-*`, `LINK-*`, `API-*`, `SAFE-*`, `SEC-*`, `LEG-*`, `QA-*`). |
| 4 | [`conventions.md`](conventions.md) | Siempre. Idioma, **reglas de ramas**, estilo Rust, errores, tests, formato del reporte. |
| 5 | [`security-model.md`](security-model.md) | Tocas logs, trazas, vouchers, transportes, la CLI o el simulador por red. |
| 6 | [`public-api.md`](public-api.md) | Tocas cualquier tipo o función pública. Es el contrato con los usuarios del crate. |
| 7 | [`implementation-status.md`](implementation-status.md) | Planificas trabajo: qué existe, qué falta, qué tarea lo hace. |
| 8 | [`handoff.md`](handoff.md) | Retomas en otra sesión/equipo: estado, pendientes, entorno y forma de trabajo. |

## Documentos relacionados

| Documento | Ubicación |
|:--|:--|
| Protocolo serial (normativo: tramas, enlace, comandos, campos, códigos) | [`docs/protocol-spec.md`](../docs/protocol-spec.md) |
| Simulador de POS (escenarios, servidor TCP) | [`docs/simulator-spec.md`](../docs/simulator-spec.md) |
| Pruebas manuales con un POS real | [`docs/hardware-testing.md`](../docs/hardware-testing.md) |
| Publicación en crates.io | [`docs/release.md`](../docs/release.md) |
| Plan de tareas | [`tasks/README.md`](../tasks/README.md) |
| Proyecto que consumirá el SDK (solo lectura) | `../rework-sistema-facturacion/` (app de escritorio Tauri, `docs/desktop-spec.md`) |
| SDKs oficiales (referencia, BSD-3) | `github.com/TransbankDevelopers/transbank-pos-sdk-{nodejs,dotnet,c,java,python}` |

## Nombres

| Concepto | Nombre aquí |
|:--|:--|
| Este repo | `transbank-SDK` (abreviatura `tbk`) |
| Crates | `transbank-pos` (lib, `use transbank_pos`), `transbank-pos-sim` (lib + bin `tbk-pos-sim`), `transbank-pos-cli` (bin `tbk-pos`) |
| El computador que usa el SDK | host |
| El terminal de Transbank | POS |
| Mensaje entre STX y ETX | payload; con STX/ETX/LRC: trama |
| Primer campo del payload | código de función (`0200`) |
| Segundo campo de una respuesta | código de respuesta (`00` = aprobado) |
