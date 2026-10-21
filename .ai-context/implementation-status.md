# Estado de implementación

Qué es real hoy versus planificado. **Lo mantiene el orquestador** a partir de los reportes de las tareas validadas. No asumas que algo funciona porque existe un archivo para ello.

Última actualización: 2026-10-21 — INFRA-01 integrada (esqueleto del workspace, sin lógica).

| Área | Estado | Tarea | Notas |
|:--|:--|:--|:--|
| Documentación normativa (`AGENTS.md`, `.ai-context/`, `docs/`, `tasks/`) | ✅ | — | Protocolo derivado de los SDKs oficiales Node/.NET/C |
| Workspace Cargo, lints, CI manual | ✅ | INFRA-01 | Licencias por crate vía symlinks; CI sin ejecutar aún (solo push a main/develop o manual) |
| LRC, tramas, decodificador | ⬜ | PROTO-01 | |
| Tipos, códigos, mensajes Integrado | ⬜ | PROTO-02 | |
| Transporte, reloj, enlace | ⬜ | LINK-01 | |
| Simulador + `tbk-pos-sim` | ⬜ | SIM-01 | |
| Cliente `PosIntegrado` | ⬜ | CLIENT-01 | |
| Serie, TCP, autoconnect | ⬜ | SERIAL-01 | |
| CLI `tbk-pos` | ⬜ | CLI-01 | |
| Rustdoc, READMEs, CHANGELOG, empaquetado | ⬜ | DOCS-01 | |
| Validación con POS real | ⬜ | HW-01 (dueño) | Preguntas OPEN-01…06 de `docs/protocol-spec.md` §10 |
| POS Autoservicio | ⬜ | AUTO-01 | |
| API async (tokio) | ⬜ | ASYNC-01 | |
| Publicación en crates.io | ⬜ | REL-01 (dueño) | Nombres libres al 2026-10-21 |

## Brechas conocidas

- El protocolo no tiene documentación oficial pública en este repo: los bytes salen de los SDKs oficiales y hay 6 puntos sin confirmar (`docs/protocol-spec.md` §10) hasta HW-01.
