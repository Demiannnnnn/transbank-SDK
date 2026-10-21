# Tarea CLI-01: CLI `tbk-pos`

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-CLI-01` |
| **Estado** | ✅ hecho |
| **Rama** | `dev/cli-01-tbk-pos` |
| **Depende de** | CLIENT-01, SERIAL-01 |
| **Restricciones** | LEG-03, SEC-02, SEC-03, QA-03 · `security-model.md` §5 · `docs/hardware-testing.md` §2 (comandos que usa el dueño) |
| **Prioridad** | P1 |

## 1. Contexto

Herramienta para probar un POS real (HW-01) y el simulador, y demo del SDK. Los comandos y flags de `docs/hardware-testing.md` §2 son obligatorios: el dueño los usará tal cual.

## 2. Requerimientos

### RF-01 Conexión (flags globales, excluyentes)
`--port <path>` (serie, `--baud`, por defecto 115200) · `--tcp <host:port>` · `--sim` (simulador en memoria, con `--scenario` igual que `tbk-pos-sim`). Sin ninguno ⇒ error claro. Además `--ack-timeout-ms`, `--response-timeout-s`, `--json`, `--show-voucher`, `--trace-file <archivo>` (instala `tracing-subscriber` con `RUST_LOG` o `transbank_pos=trace` por defecto, a archivo; activa la feature `tracing` de la librería), `--yes`.

### RF-02 Comandos
`ports`, `autoconnect`, `poll`, `load-keys`, `sale <monto> <ticket> [--status] [--voucher]`, `multicode-sale <monto> <ticket> <comercio>`, `last-sale`, `refund <operación>`, `totals`, `detail [--print]`, `close`, `normal-mode`, `raw <payload> [--expect <código>]` (envía un payload arbitrario —validado por `encode_frame`— e imprime las tramas recibidas; para OPEN-01).

### RF-03 Salida
- Humana en español: "Venta APROBADA — autorización 757752 — $12.000 — VISA crédito ···9480 — 3 cuotas de $3.334"; rechazos con el mensaje del código; mensajes intermedios en vivo con `--status`; "SIMULADOR" si el terminal empieza por `SIM` (security-model §4).
- `--json`: un objeto por respuesta (`serde_json`), con redacción salvo `--show-voucher`.
- `ResponseTimeout` en `sale`/`multicode-sale`: mensaje explícito "Resultado desconocido: NO repitas la venta; ejecuta `tbk-pos … last-sale`" (ADR-04).
- `refund`, `close`, `normal-mode` piden confirmación (`¿Seguro? [s/N]`) salvo `--yes`; sin TTY y sin `--yes` ⇒ error.
- Códigos de salida: 0 aprobado/ok · 1 respuesta no aprobada · 2 error de comunicación/protocolo · 64 uso incorrecto.

### RF-04 Ayuda
`--help` con el aviso "Herramienta NO oficial; no afiliada a Transbank" (LEG-03) y ejemplos.

## 3. Reglas

- Sin `println!` (lint `print_stdout`): escritura con `writeln!` sobre `stdout().lock()` y errores al `stderr().lock()`, propagando el error de E/S.
- Lógica de presentación en funciones puras testeables (`format_sale(&SaleResponse) -> String`).

## 4. Tests (`crates/transbank-pos-cli/tests/`, binario real con `--sim`)

- `poll`, `sale 12000 ABC123`, `last-sale`, `totals`, `detail`, `close --yes`, `refund 1 --yes` ⇒ salida esperada y código 0.
- `--scenario reject:1 sale 100 T1` ⇒ código 1 y "Rechazado".
- `--scenario no-response sale 100 T1` con `--response-timeout-s 1` ⇒ código 2 y el mensaje de ADR-04 (el test usa tiempo real corto; es aceptable aquí, ≤ 2 s).
- `--json sale …` ⇒ JSON válido sin `account_number` en claro.
- `close` sin `--yes` y sin TTY ⇒ error, no ejecuta.
- Sin flag de conexión ⇒ código 64.
- Funciones de formato con unit tests (miles, cuotas, marcas).

## 5. Definición de terminado

- [x] RF-01…RF-04.
- [x] AGENTS.md §8 en verde.
- [x] Prueba manual con `tbk-pos-sim` por TCP: transcripción en el Reporte.
- [x] Reporte completo.

## Reporte

Implementada por Claude (fallback: cuota Gemini en 0 %), 2026-10-21.

- **Conexión:** `--port` (+`--baud`), `--tcp` y `--sim` son excluyentes (clap); `--scenario` requiere `--sim`.
  - `--sim` usa `SimTransport` con `ManualClock`, así que los timeouts no consumen tiempo real.
  - El simulador en memoria **arranca con una venta de demostración** (ticket `DEMO01`, $12.000 crédito, operación 000001). Cada invocación es un proceso nuevo y la tarea pide `last-sale`/`refund 1`/`totals`/`detail` con código 0; está documentado en `--help`.
  - Flags globales: `--ack-timeout-ms`, `--response-timeout-s`, `--json`, `--show-voucher`, `--yes` y `--trace-file`. `--trace-file` usa `tracing-subscriber` sobre el archivo con `RUST_LOG` o `transbank_pos=trace` por defecto; la CLI activa las features `serde` y `tracing` de la librería.
- **Comandos:** todos los de RF-02.
  - `raw` sin `--expect` espera `código + 10`; para `0100`/`0300`/`0070` solo espera el ACK. Imprime los `0900` y la respuesta cruda.
  - `ports` y `autoconnect` no requieren conexión.
- **Salida:**
  - Humana en español con `[SIMULADOR]` cuando el terminal empieza por `SIM`.
  - `--json` pasa la respuesta por `serde_json` y redacta `account_number`, `raw` y (salvo `--show-voucher`) `voucher` a cualquier profundidad. Con `--json`, los `0900` van a stderr para que stdout sea JSON válido.
  - Timeout, E/S o `BadLrc` en `sale`/`multicode-sale` ⇒ "Resultado desconocido: NO repitas la venta; ejecuta `tbk-pos <conexión> last-sale`".
  - `refund`, `close` y `normal-mode` piden `¿Seguro? [s/N]`; sin TTY y sin `--yes` ⇒ código 64 sin ejecutar.
  - Códigos de salida: 0, 1, 2 y 64. Los errores de clap también dan 64; `--help` y `--version`, 0. `InvalidArgument` (monto, ticket, operación) da 64.
- **`tbk-pos-sim`:** el parser de escenarios pasó a la lib del simulador (`impl FromStr for Scenario`), compartido por los dos binarios.
- **Tests:**
  - 14 de integración en `tests/cli.rs` con el binario real: todos los de la sección 4, más `--show-voucher`, `raw` con el formato .NET, argumentos inválidos ⇒ 64 y `--trace-file` redactado (contiene "frame sent" y `<redacted 19>`, no la cuenta).
  - Unitarios de formato (miles, cuotas, marcas, rechazos, totales) y de redacción JSON.
  - 209 tests en total en el workspace.
- **Prueba manual con `tbk-pos-sim --listen 127.0.0.1:7782`:**
  ```
  $ tbk-pos --tcp 127.0.0.1:7782 poll                     → POS conectado [0]
  $ tbk-pos --tcp 127.0.0.1:7782 load-keys                → [SIMULADOR] Llaves cargadas — comercio 597029414300 — terminal SIM00001 [0]
  $ tbk-pos --tcp 127.0.0.1:7782 sale 1500 T00001 --status → … Esperando Leer Tarjeta / … Solicitando Confirmar Monto / … Solicitando Ingreso de Clave / … Enviando transacción al Host
                                                            [SIMULADOR] Venta APROBADA — autorización 112648 — $1.500 — VISA crédito ···0037 — operación 000001 [0]
  $ tbk-pos --tcp 127.0.0.1:7782 sale 3001 T00002         → … Redcompra débito ···0074 — operación 000002 [0]
  $ tbk-pos --tcp 127.0.0.1:7782 last-sale                → Última venta APROBADA … operación 000002 [0]
  $ tbk-pos --tcp 127.0.0.1:7782 refund 2 --yes           → Anulación RECHAZADA — 08 No puede Anular Transacción Debito [1]
  $ tbk-pos --tcp 127.0.0.1:7782 refund 1 --yes           → Anulación APROBADA — autorización 128486 — operación 000003 [0]
  $ tbk-pos --tcp 127.0.0.1:7782 totals                   → 1 venta — total $3.001 [0]
  $ tbk-pos --tcp 127.0.0.1:7782 detail                   → 2 líneas (T00001 crédito, T00002 débito) [0]
  $ tbk-pos --tcp 127.0.0.1:7782 close --yes              → [SIMULADOR] Cierre APROBADO … [0]
  $ tbk-pos ports                                          → /dev/cu.debug-console, /dev/cu.Bluetooth-Incoming-Port (sin duplicados tty.*) [0]
  ```
- AGENTS.md §8: los cinco comandos en verde; `validate.sh` OK.
