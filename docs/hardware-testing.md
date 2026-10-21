# Pruebas con un POS real (HW-01)

Solo las hace **el dueño** (o un contribuidor con su propio terminal). Ningún agente conecta hardware (AGENTS.md §7). Objetivo: cerrar las preguntas abiertas de `docs/protocol-spec.md` §10 y validar antes de publicar en crates.io (`docs/release.md`).

## 1. Requisitos

- POS Integrado de Transbank (VX520/VX520c o Desk 3500) **en modo integrado** (lo activa Transbank a pedido del comercio) y con contrato vigente.
- Cable USB (o RS-232) al computador. Linux: usuario en `dialout`/`uucp`. macOS: el puerto aparece como `/dev/cu.usbmodem*`. Windows: `COMx` en el Administrador de dispositivos (driver del fabricante si hace falta).
- Una tarjeta de **crédito** propia (para poder anular el mismo día) y una de débito.
- `tbk-pos` compilado de `develop-AGENTS`: `cargo build -p transbank-pos-cli --release`.
- Trazas: `RUST_LOG=transbank_pos=trace` con `--trace-file hw-<fecha>.log` (la CLI redacta datos sensibles).

## 2. Lista de verificación

Anota en cada paso: resultado, código de respuesta y la línea de la traza. **Montos mínimos** ($50–$100) y anular lo cobrado al final.

| # | Paso | Comando | Esperado | Cierra |
|:--|:--|:--|:--|:--|
| 1 | Listar puertos | `tbk-pos ports` | Aparece el POS (USB, VID/PID) | — |
| 2 | Poll | `tbk-pos --port <p> poll` | "POS conectado" | OPEN-05 (¿basura al abrir?) |
| 3 | Autodetección | `tbk-pos autoconnect` | Encuentra el puerto | — |
| 4 | Cargar llaves | `tbk-pos --port <p> load-keys` | Código 00 | — |
| 5 | Venta crédito $100 con estados | `tbk-pos --port <p> sale 100 T00001 --status` | Mensajes intermedios + aprobada | OPEN-02 (¿ACK a 0900?) |
| 6 | Venta con formato .NET | `tbk-pos --port <p> raw '0200|100|T00002||0|1|'` | ¿La acepta? | OPEN-01 |
| 7 | Venta con formato Node | `tbk-pos --port <p> raw '0200|000000100|T00003|||0'` | ¿La acepta? | OPEN-01 |
| 8 | Ticket en minúsculas | `tbk-pos --port <p> sale 100 abc123` | ¿Aparece así en el voucher? | OPEN-04 |
| 9 | Venta cancelada en el POS | `sale 100 T00004` y cancelar en el teclado | Código 7, `Ok` con `is_approved=false` | — |
| 10 | Timeout | `sale 100 T00005` y no pasar la tarjeta | `ResponseTimeout` o código del POS; luego `last-sale` | — |
| 11 | Última venta | `tbk-pos --port <p> last-sale` | Coincide con la última aprobada | OPEN-06 |
| 12 | Venta débito $100 | `sale 100 T00006` con débito | Aprobada, `DB` | — |
| 13 | Anular débito | `refund <op del 12>` | Código 8 | — |
| 14 | Anular crédito | `refund <op del 5>` | Código 00 | — |
| 15 | Totales | `tbk-pos --port <p> totals` | Cantidad y total coherentes | — |
| 16 | Detalle al host | `tbk-pos --port <p> detail` | Lista de ventas; ¿cuántas tramas vacías al final? | OPEN-03 |
| 17 | Detalle impreso | `tbk-pos --port <p> detail --print` | Imprime en el POS | — |
| 18 | Cierre | `tbk-pos --port <p> close --yes` | Código 00, imprime | OPEN-06 |
| 19 | Desconectar el cable a mitad de una venta | — | `Io`, la app no se cuelga | — |
| 20 | Windows | pasos 1, 2, 5 en Windows 10/11 | Igual | — |

## 3. Después

- Subir la traza **anonimizada** a `crates/transbank-pos/tests/fixtures/hw/<modelo>-<fecha>.txt` (rama `dev/hw-01-traces`): commerce code reemplazado por `597029414300`, sin vouchers, cuentas enmascaradas.
- El orquestador actualiza `docs/protocol-spec.md` §10 (preguntas cerradas, con la evidencia) y crea tareas `fix` si el comportamiento real difiere.
- Con todo en verde, el dueño decide la publicación (`docs/release.md`).
