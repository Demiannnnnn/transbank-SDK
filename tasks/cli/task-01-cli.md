# Tarea CLI-01: CLI `tbk-pos`

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-CLI-01` |
| **Estado** | ⬜ pendiente |
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

- [ ] RF-01…RF-04.
- [ ] AGENTS.md §8 en verde.
- [ ] Prueba manual con `tbk-pos-sim` por TCP: transcripción en el Reporte.
- [ ] Reporte completo.

## Reporte

_(pendiente)_
