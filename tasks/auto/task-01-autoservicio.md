# Tarea AUTO-01: POS Autoservicio (mensajes, cliente, simulador y CLI)

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-AUTO-01` |
| **Estado** | ✅ terminada |
| **Rama** | `dev/auto-01-autoservicio` |
| **Depende de** | CLIENT-01, SIM-01 (y CLI-01 para RF-04) |
| **Restricciones** | PROTO-01, PROTO-05…08, API-01…07, QA-01, QA-02 · `docs/protocol-spec.md` §7, §9 · `docs/simulator-spec.md` §3 · `public-api.md` §5, §8 |
| **Prioridad** | P2 |

## 1. Contexto

Kioscos y máquinas de autoatención. Mismo enlace, otros comandos y otro layout de respuesta (protocol-spec §7). PROTO-08: módulo propio, sin `if` por modelo en los parsers de Integrado.

## 2. Requerimientos

### RF-01 Mensajes (`message/autoservicio.rs`)
Payloads de §7 (venta `0200|{amount:09}|{ticket:06}|{voucher}|{status}`: mismo relleno que Integrado, aunque los ejemplos de los SDKs no rellenan el monto; queda como pregunta para HW-01, anótala en el Reporte) y parsers: `SelfServiceSaleResponse` (layout §7.1 con `installments_type`, `installments_type_description`, voucher en 15), `SelfServiceMultiCodeSaleResponse` (+20 `commerce_provider_code`), `InitializationResponse` (`1080`: fecha, hora; `90` ⇒ `is_approved`), `parse_close` con voucher en 4, refund `1210`.

### RF-02 Cliente `PosAutoservicio`
Métodos de `public-api.md` §8 (`poll`, `load_keys`, `initialize`, `initialization_result`, `sale`, `sale_with_status`, `multicode_sale`, `last_sale(voucher)`, `refund_last`, `close(voucher)`). Constructor `open(path)` con 19200 por defecto (feature `serial`).

### RF-03 Simulador
`TerminalKind::Autoservicio` completo (simulator-spec §3, último párrafo); `tbk-pos-sim --kind autoservicio`.

### RF-04 CLI
`tbk-pos --autoservicio …` cambia el cliente (baud por defecto 19200); comandos `init`, `init-result` y los equivalentes; `--voucher` imprime el voucher en líneas de 40.

## 3. Tests

- Payloads exactos y fixtures de §7 (incluido `0210|00|597029414300|IM750164|123456|776549|10000|6590|58|CR|||VI|18032026|123506||03|03|3334|CUOTAS SIN INTERES` y `1080|90|03022026|111543`).
- Cliente contra simulador: cada método, venta rechazada, voucher de 3 líneas.
- CLI `--autoservicio --sim sale 1000 T1` ⇒ aprobada.

## 4. Definición de terminado

- [ ] RF-01…RF-04.
- [ ] AGENTS.md §8 en verde.
- [ ] Reporte completo.

## Reporte

- **Estado:** done
- **Rama / commits:** `dev/auto-01-autoservicio` · `f6c15db`, `b55e096`, `cb3a721`, `b73be65`, `93e080f`
- **Hecho:**
  - `transbank-pos`: Módulo `message::autoservicio` con payloads de §7 (`0070`, `0080`, `0200`, `0270`, `0250`, `1200`, `0500`), structs tipados (`SelfServiceSaleResponse`, `SelfServiceMultiCodeSaleResponse`, `InitializationResponse`) y parsers con módulo propio (PROTO-08). Códigos de función `FC_INITIALIZE`, `FC_INITIALIZATION_RESULT`, `FC_INITIALIZATION_RESPONSE`.
  - `transbank-pos`: Soporte de redacción de trazas en `link.rs` con `TraceLayout::Autoservicio` (índices `[11, 15]` en ventas, `[4]` en cierre, SEC-02) sin alterar la API pública de `Link`.
  - `transbank-pos`: Cliente `PosAutoservicio<T, C>` con el conjunto exacto de operaciones de `public-api.md` §8 y constructor `open` a 19200 baudios con feature `serial`. Re-exportaciones públicas en la raíz del crate.
  - `transbank-pos-sim`: Soporte completo para `TerminalKind::Autoservicio` con parsing estricto según §7 y respuestas de venta, última venta, anulación de última venta (`1200`), cierre y rechazo de comandos exclusivos de Integrado (`0700`... -> código 12).
  - `transbank-pos-cli`: Flag `--autoservicio`, baud por defecto 19200, comandos `init`, `init-result`, `refund-last`, `--voucher` en `last-sale` y `close`, validaciones de comandos no soportados con salida 64 y módulo `autoservicio.rs`.
  - Tests exhaustivos: `fixtures/autoservicio.txt`, `autoservicio_messages.rs`, `client_autoservicio.rs`, y `cli_autoservicio.rs`.
- **Archivos principales:**
  - `crates/transbank-pos/src/message/autoservicio.rs`
  - `crates/transbank-pos/src/client/autoservicio.rs`
  - `crates/transbank-pos/src/link.rs`
  - `crates/transbank-pos-sim/src/request.rs`
  - `crates/transbank-pos-sim/src/simulator.rs`
  - `crates/transbank-pos-cli/src/autoservicio.rs`
  - `crates/transbank-pos-cli/src/main.rs`
  - `crates/transbank-pos/tests/autoservicio_messages.rs`
  - `crates/transbank-pos/tests/client_autoservicio.rs`
  - `crates/transbank-pos-cli/tests/cli_autoservicio.rs`
- **Comandos:** fmt ✅ · clippy ✅ · clippy no-default ✅ · test ✅ (328 tests) · doc ✅
- **Decisiones:**
  - `SelfServiceSaleResponse::is_approved()` y `SelfServiceMultiCodeSaleResponse::is_approved()` verifican estrictamente `response_code == ResponseCode::Approved` (código `00`). El código `90` en `0210` no es aprobado.
  - `InitializationResponse::is_approved()` verifica estrictamente `response_code == ResponseCode::InitializationOk` (código `90`). El código `00` en `1080` no es aprobado.
  - `TraceLayout::Autoservicio` redacta los campos 11 y 15 en `0210`, `0260` y `0271`; campo 4 en `0510`; y nada en `0810`, `1210`, `0900`, `1080`.
  - Reemplazo autorizado del test `autoservicio_is_pending` en el simulador por `autoservicio_answers_poll_and_rejects_integrado_only_commands`.
  - Pregunta abierta para HW-01: Relleno de ceros en monto en solicitud de venta Autoservicio (`0200|000012000|...` vs `0200|12000|...`). La librería envía con 9 dígitos rellenados `{amount:09}` al igual que Integrado. El simulador acepta montos con o sin relleno. Debe validarse contra un POS Autoservicio real en HW-01.
- **Pendiente / bloqueos:** ninguno
- **Desviaciones de la spec o de .ai-context:** ninguna
