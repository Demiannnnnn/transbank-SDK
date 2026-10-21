# Estado de implementación

Qué es real hoy versus planificado. **Lo mantiene el orquestador** a partir de los reportes de las tareas validadas. No asumas que algo funciona porque existe un archivo para ello.

Última actualización: 2026-10-21 — núcleo integrado hasta CLI-01 (falta DOCS-01; luego HW-01 del dueño).

| Área | Estado | Tarea | Notas |
|:--|:--|:--|:--|
| Documentación normativa (`AGENTS.md`, `.ai-context/`, `docs/`, `tasks/`) | ✅ | — | Protocolo derivado de los SDKs oficiales Node/.NET/C |
| Workspace Cargo, lints, CI manual | ✅ | INFRA-01 | Licencias por crate vía symlinks; CI sin ejecutar aún (solo push a main/develop o manual) |
| LRC, tramas, decodificador | ✅ | PROTO-01 | Vectores §2.3 byte a byte; proptest de partición, ida y vuelta y bytes arbitrarios |
| Tipos, códigos, mensajes Integrado | ✅ | PROTO-02 | 56 códigos; parsers con fixtures de §5; `Debug` redacta cuenta, voucher y raw |
| Transporte, reloj, enlace | ✅ | LINK-01 | ACK implícito si llega trama sin ACK; `BadLrc` al 4.º seguido; tiempo simulado |
| Simulador + `tbk-pos-sim` | ✅ | SIM-01 | Integrado completo; Autoservicio responde 12 (pendiente AUTO-01) |
| Cliente `PosIntegrado` | ✅ | CLIENT-01 | Todos los métodos salvo `open`/`autoconnect`; tests end-to-end contra el simulador |
| Serie, TCP, autoconnect | ✅ | SERIAL-01 | TCP probado end-to-end; serie solo compila y lógica pura (hardware en HW-01) |
| CLI `tbk-pos` | ✅ | CLI-01 | Todos los comandos de `docs/hardware-testing.md`; `--sim` con venta de demostración |
| Rustdoc, READMEs, CHANGELOG, empaquetado | ⬜ | DOCS-01 | |
| Validación con POS real | ⬜ | HW-01 (dueño) | Preguntas OPEN-01…06 de `docs/protocol-spec.md` §10 |
| POS Autoservicio | ⬜ | AUTO-01 | |
| API async (tokio) | ⬜ | ASYNC-01 | |
| Publicación en crates.io | ⬜ | REL-01 (dueño) | Nombres libres al 2026-10-21 |

## Brechas conocidas

- El protocolo no tiene documentación oficial pública en este repo: los bytes salen de los SDKs oficiales y hay 6 puntos sin confirmar (`docs/protocol-spec.md` §10) hasta HW-01.
