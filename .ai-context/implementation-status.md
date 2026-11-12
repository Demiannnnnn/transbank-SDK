# Estado de implementación

Qué es real hoy versus planificado. **Lo mantiene el orquestador** a partir de los reportes de las tareas validadas. No asumas que algo funciona porque existe un archivo para ello.

Última actualización: 2026-11-05 — AUTO-01 y ASYNC-01 integradas sobre la 1.0.0 (aditivas, sin publicar ni tag), 396 tests. Falta HW-01 (dueño).

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
| API async (tokio) | ✅ | ASYNC-01 | `AsyncLink`, `AsyncPosIntegrado`, `AsyncSimStream`; lógica de decisión compartida con `Link` en `exchange.rs`. 396 tests del workspace. `open_serial` solo compila y se prueba su error: sin puerto real. Sin cliente async de Autoservicio |
| Publicación en crates.io | ⬜ | REL-01 (dueño) | Nombres libres al 2026-10-21 |

## Brechas conocidas

- **`SaleResponse::is_approved()` de Integrado (1.0.0) aceptaba el código 90**, porque usaba `ResponseCode::is_approved()`, aunque §9 limita el 90 a la inicialización. Propuesta en `dev/fix-sale-approved-proposal`, pendiente del dueño. Los tipos Autoservicio ya son estrictos.
- `cargo-mutants` no está instalado en este equipo y AGENTS.md §7 prohíbe `cargo install`: AUTO-01 y ASYNC-01 tuvieron mutaciones manuales en lo crítico (aprobación, redacción de trazas, drenado, EOF, ACK en la cola del detalle, plazos), no una pasada completa.
- El MSRV 1.85 solo se comprobó en los `rust-version` declarados en `Cargo.lock` (ninguno supera 1.85): no se compiló con 1.85 porque este equipo tiene Rust 1.98.
- Nada se probó con un POS real: el puerto serie solo se compiló; TCP y toda la lógica se probaron contra el simulador.
- El CI de GitHub Actions no se ha ejecutado nunca (se dispara con push a `main`/`develop` o a mano): Linux/Windows sin verificar.
- El protocolo no tiene documentación oficial pública en este repo: los bytes salen de los SDKs oficiales y hay 6 puntos sin confirmar (`docs/protocol-spec.md` §10) hasta HW-01.
