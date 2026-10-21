# Tarea CLIENT-01: Cliente `PosIntegrado`

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-CLIENT-01` |
| **Estado** | ✅ hecho |
| **Rama** | `dev/client-01-pos-integrado` |
| **Depende de** | PROTO-02, LINK-01, SIM-01 |
| **Restricciones** | API-01…07, LINK-03, QA-02, ADR-03, ADR-04 · `public-api.md` §8 · `docs/protocol-spec.md` §4, §8 |
| **Prioridad** | P0 |

## 1. Contexto

La API que usa la gente. Es delgada: valida, arma el payload con `message`, llama al `Link` y parsea. Su valor está en la ergonomía, la documentación y los tests end-to-end contra el simulador.

## 2. Requerimientos

### RF-01 `client/integrado.rs`
`PosIntegrado<T, C>` con **todos** los métodos de `public-api.md` §8 (salvo `open`/`autoconnect`, que son de SERIAL-01). Cada método:
- usa el payload y el parser de PROTO-02 y el código de respuesta esperado correcto;
- `sales_detail(PrintOnPos)` usa `send_command` y devuelve `Ok(vec![])`; `SendToHost` usa `request_list` y parsea cada entrada;
- rustdoc con `# Errors` y, en `sale`, una sección `# Timeouts y doble cobro` que explica `last_sale()` (ADR-04) con ejemplo.

### RF-02 Re-exports
`lib.rs` re-exporta en la raíz todo lo marcado **[raíz]** en `public-api.md`. Ejemplo de crate (`//!`) con `MemoryTransport`/simulador que compila como doctest.

### RF-03 Ejemplos
`crates/transbank-pos/examples/{poll.rs, sale.rs, detail.rs}` que funcionan contra el simulador TCP (`TBK_POS_TCP=127.0.0.1:7777`) — `TcpTransport` llega en SERIAL-01: hasta entonces los ejemplos usan `SimTransport` en memoria y SERIAL-01 los cambia. Documenta en el Reporte.

## 3. Reglas

- Sin lógica de protocolo nueva en el cliente: si falta algo, va en `message` o `link` (mínimo y justificado).
- LINK-03: ningún método reintenta una venta, multicódigo o anulación.

## 4. Tests (`crates/transbank-pos/tests/client_integrado.rs`, contra `SimTransport`)

Para **cada** método público: caso feliz con asserts sobre los campos. Además:
- venta rechazada (`RejectNextSale(1)`) ⇒ `Ok`, `!is_approved()`;
- `sale_with_status` con `Intermediate(vec![78,81,82])` ⇒ callback recibe los tres;
- `NoResponse` ⇒ `ResponseTimeout` y luego `last_sale()` devuelve la venta registrada por el simulador (flujo de ADR-04);
- `NakNext(2)` ⇒ venta aprobada; `DropAcks(3)` ⇒ `NoAck`;
- `CorruptLrc(1)` ⇒ venta aprobada;
- `WrongFunctionCode("0810")` ⇒ `UnexpectedResponse`;
- `RequireClose` ⇒ venta `Ok` con `requires_close()`; `close()` y luego venta aprobada;
- `refund` de crédito ⇒ aprobada; de débito ⇒ código 8;
- `sales_detail(SendToHost)` tras 3 ventas ⇒ 3 entradas en orden; sin ventas ⇒ vacío;
- `totals` coherente con las ventas;
- `PosIntegrado<SimTransport>: Send`.

## 5. Definición de terminado

- [x] RF-01…RF-03.
- [x] AGENTS.md §8 en verde (doctests incluidos).
- [x] Reporte completo.

## Reporte

Implementada por Claude (fallback: cuota Gemini en 0 %), 2026-10-21.

- `client/integrado.rs`: `PosIntegrado<T, C>` con todos los métodos de `public-api.md` §8 salvo `open`/`autoconnect` (SERIAL-01). Cada método usa el payload, el parser y el `FC_*` de PROTO-02.
  - `sale`/`multicode_sale` envían `status = 0`; las variantes `_with_status`, `status = 1`.
  - `sales_detail(PrintOnPos)` usa `send_command` y devuelve `vec![]`.
  - `sale` tiene la sección "Timeouts y doble cobro" con un doctest que ejecuta el flujo de ADR-04 contra el simulador.
  - Sin lógica nueva en `message` ni en `link`.
- Reexporta `PosIntegrado` en la raíz. El ejemplo de crate (`//!`) corre contra el simulador como doctest.
- **Dev-dependency `transbank-pos-sim` solo por `path` (sin versión):**
  - El ciclo de dev-dependency funciona en tests de integración, doctests y ejemplos (los tipos se unifican).
  - `cargo package` quita esa dependencia del manifiesto, así que `transbank-pos` se puede publicar antes que el simulador.
  - **Para DOCS-01:** el `.crate` incluye `tests/` y `examples/`, que usan el simulador. No compilan sin él, pero `cargo publish` solo compila la lib. Conviene excluirlos del paquete o dejarlos y documentarlo.
- **Ejemplos:** `examples/{poll,sale,detail}.rs` corren contra `SimTransport` en memoria; SERIAL-01 los pasa a `TBK_POS_TCP`/`TBK_POS_PORT`.
  - `sale` muestra los `0900` y "Aprobada: $12.000 autorización 112648 operación 000001", y ante un timeout consulta `last_sale`.
  - `detail` lista 2 ventas y los totales.
- **Tests:** 17 en `tests/client_integrado.rs`.
  - Un caso feliz por cada método público.
  - Todos los casos de la sección 4: rechazo, `0900` (78, 81, 82), `NoResponse` ⇒ `ResponseTimeout` a los 150 s simulados y `last_sale` resuelve sin reenviar la venta, `NakNext(2)`, `DropAcks(3)` ⇒ `NoAck` a los 6 s, `CorruptLrc(1)`, `WrongFunctionCode`, `RequireClose` + `close`, anulación de crédito y de débito (8), detalle vacío y de 3 ventas en orden, totales.
  - `Send`.
  - 172 tests en total en el workspace.
- AGENTS.md §8: los cinco comandos en verde; `validate.sh` OK.
