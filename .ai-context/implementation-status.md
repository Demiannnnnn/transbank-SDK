# Estado de implementación

Qué es real hoy versus planificado. **Lo mantiene el orquestador** a partir de los reportes de las tareas validadas. No asumas que algo funciona porque existe un archivo para ello.

Última actualización: 2026-10-21 — **núcleo completo** (INFRA-01 … DOCS-01) en `develop-AGENTS`, 211 tests. Falta HW-01 (dueño), AUTO-01 y ASYNC-01. Implementado por Claude (cuota Gemini en 0 %).

| Área | Estado | Tarea | Notas |
|:--|:--|:--|:--|
| Documentación normativa (`AGENTS.md`, `.ai-context/`, `docs/`, `tasks/`) | ✅ | — | Protocolo derivado de los SDKs oficiales Node/.NET/C |
| Workspace Cargo, lints, CI manual | ✅ | INFRA-01 | CI sin ejecutar aún (solo push a `main`/`develop` o manual) |
| LRC, tramas, decodificador | ✅ | PROTO-01 | Vectores §2.3 byte a byte; proptest de partición, ida y vuelta y bytes arbitrarios |
| Tipos, códigos, mensajes Integrado | ✅ | PROTO-02 | 56 códigos; parsers con fixtures de §5; `Debug` redacta cuenta, voucher y raw |
| Transporte, reloj, enlace | ✅ | LINK-01 | ACK implícito si llega trama sin ACK; `BadLrc` al 4.º seguido; tiempo simulado |
| Simulador + `tbk-pos-sim` | ✅ | SIM-01 | Integrado completo; Autoservicio responde 12 (pendiente AUTO-01) |
| Cliente `PosIntegrado` | ✅ | CLIENT-01 | Todos los métodos salvo `open`/`autoconnect`; tests end-to-end contra el simulador |
| Serie, TCP, autoconnect | ✅ | SERIAL-01 | TCP probado end-to-end; serie solo compila y lógica pura (hardware en HW-01) |
| CLI `tbk-pos` | ✅ | CLI-01 | Todos los comandos de `docs/hardware-testing.md`; `--sim` con venta de demostración |
| Rustdoc, READMEs, CHANGELOG, empaquetado | ✅ | DOCS-01 | `publish --dry-run` de la lib OK (7 avisos por tests excluidos del paquete); licencias copiadas + test |
| Validación con POS real | ⬜ | HW-01 (dueño) | Preguntas OPEN-01…06 de `docs/protocol-spec.md` §10 |
| POS Autoservicio | ⬜ | AUTO-01 | |
| API async (tokio) | ⬜ | ASYNC-01 | |
| Publicación en crates.io | ⬜ | REL-01 (dueño) | Nombres libres al 2026-10-21 |

## Brechas conocidas

- Nada se probó con un POS real: el puerto serie solo se compiló; TCP y toda la lógica se probaron contra el simulador.
- El CI de GitHub Actions no se ha ejecutado nunca (se dispara con push a `main`/`develop` o a mano): Linux/Windows sin verificar.
- El protocolo no tiene documentación oficial pública en este repo: los bytes salen de los SDKs oficiales y hay 6 puntos sin confirmar (`docs/protocol-spec.md` §10) hasta HW-01.
