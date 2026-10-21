# Modelo de seguridad — transbank-SDK

## 1. Qué protege y qué no

El SDK no maneja secretos: no hay llaves, contraseñas ni datos completos de tarjeta (el POS cifra y habla con Transbank por su cuenta). Los riesgos reales son:

| Riesgo | Impacto | Mitigación |
|:--|:--|:--|
| **Doble cobro** por reintentar una venta sin respuesta | Dinero del cliente | ADR-04, LINK-03; la API documenta `last_sale()`; la CLI lo sugiere |
| **Venta registrada como aprobada sin estarlo** (parsear la respuesta equivocada, aceptar LRC malo) | Pérdida del comercio | LINK-01, LINK-06, PROTO-05 |
| **Fuga de datos parciales** de tarjeta o vouchers en logs | Privacidad / PCI del comercio | SEC-02, SEC-03, §3 |
| **Pánico o cuelgue** del software de caja por bytes basura del puerto | Caja detenida | PROTO-04, SAFE-02, timeouts acotados |
| **Simulador expuesto en la red** aceptado como POS real | Ventas "aprobadas" falsas | SEC-04, §4 |
| **Cadena de suministro** (crates maliciosos) | Código en cajas de comercios | Lista cerrada de dependencias (architecture §3), `Cargo.lock` versionado |

## 2. Datos que entrega el POS

| Dato | Sensibilidad | Tratamiento |
|:--|:--|:--|
| `last4_digits`, `card_type`, `card_brand`, montos, fechas, autorización, operación | Baja (aparece en el voucher del cliente) | Normal |
| `account_number` (viene enmascarado por el POS, p. ej. `  ********331      `) | Media | Redactado en `Debug` y trazas: se muestra solo el largo |
| `voucher` | Media (datos del comercio y de la transacción) | Redactado en `Debug` y trazas: `voucher: <N chars>` |
| `commerce_code`, `terminal_id` | Baja | Normal |

## 3. Trazas (feature `tracing`)

- Target `transbank_pos::link`.
- `debug`: eventos del enlace sin payload — "frame sent (fc=0200, len=27, attempt=1)", "ACK received", "NAK received", "bad LRC", "response received (fc=0210)", "timeout".
- `trace`: payload completo **redactado**: campos de `account_number` y voucher reemplazados por `<redacted N>` según el layout del código de función; si el código es desconocido, solo largo y código de función.
- Nunca `info`/`warn`/`error` desde la librería: la app decide qué loguear.
- Sin la feature, la librería no escribe nada en ningún lado (sin `eprintln!`).

## 4. Simulador y TCP

- `tbk-pos-sim` escucha por defecto en `127.0.0.1`. `--listen 0.0.0.0:…` exige `--allow-remote` e imprime un aviso. Sin autenticación (es una herramienta de desarrollo).
- Las respuestas del simulador llevan `terminal_id` `SIM00001` y el código de comercio de prueba `597029414300`; la CLI muestra "SIMULADOR" cuando el `terminal_id` empieza por `SIM`.
- `TcpTransport` de la librería conecta a la dirección que da la app (para puentes serie-red); no resuelve nada por su cuenta ni abre puertos de escucha.

## 5. CLI

- `--json` imprime la respuesta completa pero con los mismos campos redactados que `Debug`, salvo `--show-voucher` explícito (para imprimir el voucher).
- Nunca guarda archivos salvo `--out <archivo>` explícito.
- Antes de `refund` y `close` pide confirmación interactiva salvo `--yes`.

## 6. Hardware real

Solo el dueño (o un contribuidor con su propio terminal) conecta un POS real, siguiendo `docs/hardware-testing.md`, con montos mínimos y anulando lo cobrado. Las trazas que se suban al repo se anonimizan (commerce code de prueba, cuentas enmascaradas, sin vouchers reales).
