# Estado de implementación

Qué es real hoy versus planificado. **Lo mantiene el orquestador** a partir de los reportes de las tareas validadas. No asumas que algo funciona porque existe un archivo para ello.

Última actualización: 2026-11-05 — AUTO-01 integrada sobre la 1.0.0 (aditiva, sin publicar ni tag). Falta HW-01 (dueño) y ASYNC-01.

| Área | Estado | Tarea | Notas |
|:--|:--|:--|:--|
| Documentación normativa (`AGENTS.md`, `.ai-context/`, `docs/`, `tasks/`) | ✅ | — | Protocolo derivado de los SDKs oficiales Node/.NET/C |
| Workspace Cargo, lints, CI manual | ✅ | INFRA-01 | CI sin ejecutar aún (solo push a `main`/`develop` o manual) |
| LRC, tramas, decodificador | ✅ | PROTO-01 | Vectores §2.3 byte a byte; proptest de partición, ida y vuelta y bytes arbitrarios |
| Tipos, códigos, mensajes Integrado | ✅ | PROTO-02 | 56 códigos; parsers con fixtures de §5; `Debug` redacta cuenta, voucher y raw |
| Transporte, reloj, enlace | ✅ | LINK-01 | ACK implícito si llega trama sin ACK; `BadLrc` al 4.º seguido; tiempo simulado |
| Simulador + `tbk-pos-sim` | ✅ | SIM-01, AUTO-01 | Integrado y Autoservicio; lo que está fuera de §7 responde 12 |
| Cliente `PosIntegrado` | ✅ | CLIENT-01 | Todos los métodos salvo `open`/`autoconnect`; tests end-to-end contra el simulador |
| Serie, TCP, autoconnect | ✅ | SERIAL-01 | TCP probado end-to-end; serie solo compila y lógica pura (hardware en HW-01) |
| CLI `tbk-pos` | ✅ | CLI-01 | Todos los comandos de `docs/hardware-testing.md`; `--sim` con venta de demostración |
| Rustdoc, READMEs, CHANGELOG, empaquetado | ✅ | DOCS-01 | `publish --dry-run` de la lib OK (7 avisos por tests excluidos del paquete); licencias copiadas + test |
| Endurecimiento de tests | ✅ | TEST-01 | 271 tests (propiedades, `serde`, casos borde del enlace, modelo vs simulador, CLI). `cargo-mutants`: 654 mutantes, 0 escapes (561 detectados, 3 por timeout, 90 no compilan) |
| Validación con POS real | ⬜ | HW-01 (dueño) | Preguntas OPEN-01…06 de `docs/protocol-spec.md` §10 |
| POS Autoservicio | ✅ | AUTO-01 | Cliente, simulador y CLI; solo contra el simulador. 328 tests del workspace. Comandos solo los de `protocol-spec.md` §7; preguntas OPEN-07 y OPEN-08 |
| API async (tokio) | ⬜ | ASYNC-01 | |
| Publicación en crates.io | ⬜ | REL-01 (dueño) | Nombres libres al 2026-10-21 |

## Brechas conocidas

- **`SaleResponse::is_approved()` de Integrado (1.0.0) acepta el código 90**, porque usa `ResponseCode::is_approved()`, aunque §9 limita el 90 a la inicialización. No se cambió: está fuera de AUTO-01 y decide el dueño. Los tipos Autoservicio ya son estrictos.
- `cargo-mutants` no está instalado en este equipo y AGENTS.md §7 prohíbe `cargo install`: AUTO-01 tuvo mutaciones manuales en lo crítico (aprobación, redacción de trazas), no una pasada completa.
- Nada se probó con un POS real: el puerto serie solo se compiló; TCP y toda la lógica se probaron contra el simulador.
- El CI de GitHub Actions no se ha ejecutado nunca (se dispara con push a `main`/`develop` o a mano): Linux/Windows sin verificar.
- El protocolo no tiene documentación oficial pública en este repo: los bytes salen de los SDKs oficiales y hay 6 puntos sin confirmar (`docs/protocol-spec.md` §10) hasta HW-01.
