# Tarea PROTO-02: Tipos, códigos, requests y respuestas del POS Integrado

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-PROTO-02` |
| **Estado** | ⬜ pendiente |
| **Rama** | `dev/proto-02-integrado-messages` |
| **Depende de** | PROTO-01 |
| **Restricciones** | PROTO-01, PROTO-05…08, API-02…06, SEC-03, LEG-02, QA-01 · `docs/protocol-spec.md` §4, §5, §6, §9 · `public-api.md` §2–§5 |
| **Prioridad** | P0 |

## 1. Contexto

Capa de mensajes sans-IO del POS Integrado: tipos validados, códigos de respuesta, construcción de cada payload y parseo tipado de cada respuesta. Sin enlace ni E/S. El cliente (CLIENT-01) y el simulador (SIM-01) la usan.

## 2. Requerimientos

### RF-01 Tipos (`protocol/types.rs`)
`Amount`, `Ticket`, `OperationNumber`, `CommerceCode`, `PosDate`, `PosTime`, `CardType`, `CardBrand` según `public-api.md` §2, con las validaciones de protocol-spec §4.1. `Display` de `Amount` con separador de miles chileno (`$12.000`). Con feature `serde`: derives (CardType/CardBrand se serializan como su código: `"CR"`, `"VI"`, o el texto de `Other`).

### RF-02 Códigos (`codes.rs`)
`ResponseCode` con **una variante por cada código** de protocol-spec §9 (nombre en inglés descriptivo) + `Other(u16)`; `from_code`, `code`, `message` (texto exacto de la tabla, comentario de origen LEG-02: `responseCodes.js` del SDK Node), `is_approved`, `requires_close`, `requires_load_keys`, `Display`. `IntermediateStatus` (§6). Test de ida y vuelta `from_code(c).code() == c` para todos los códigos de la tabla y para un `Other`.

### RF-03 Campos (`protocol/fields.rs`, `pub(crate)`)
Helpers sobre `&[&str]` ya divididos por `|`: `str_at(i) -> Option<String>` (trim; vacío ⇒ `None`), `u32_at(i, name)`, `u64_at`, `date_at`, `time_at`, `card_type_at`, `card_brand_at` con las reglas de protocol-spec §5 (PROTO-05). `required_code(fields)` lee el campo 1 (`ResponseCode`), error `Parse` si falta o no es numérico.

### RF-04 Requests (`message/integrado.rs`)
`SaleRequest`, `MultiCodeSaleRequest`, `SalesDetailMode` (`public-api.md` §4) y una función `pub fn …_payload(…) -> String` por comando de protocol-spec §4: `poll_payload`, `load_keys_payload`, `sale_payload(req, status)`, `multicode_sale_payload(req, status)`, `last_sale_payload`, `multicode_last_sale_payload(voucher)`, `refund_payload(op)`, `totals_payload`, `sales_detail_payload(mode)`, `multicode_sales_detail_payload(mode)`, `close_payload`, `normal_mode_payload`. Formato **exacto** de §4 (decisión de OPEN-01: `0200|{amount:09}|{ticket:06}||{voucher}|{status}`).
Constantes de códigos de función (`FC_*`) para requests y respuestas esperadas.

### RF-05 Respuestas (`message/responses.rs`)
Structs de `public-api.md` §5 (Integrado) y parsers `pub fn parse_sale(payload: &str) -> Result<SaleResponse>` (acepta `0210` y `0260`), `parse_multicode_sale` (`0271`, `0281`), `parse_detail_entry` (`0261`), `parse_multicode_detail_entry` (`0291`), `parse_load_keys` (`0810`), `parse_close` (`0510`), `parse_totals` (`0710`), `parse_refund` (`1210`), `parse_intermediate` (`0900`). Cada parser verifica el código de función (si no ⇒ `UnexpectedResponse`). Índices **exactos** de protocol-spec §5. `raw` = payload completo. `voucher_lines()` (§5.1).
`Debug` manual que redacta `account_number` y `voucher` (SEC-03): muestra `<redacted N chars>`.
`is_detail_terminator(&DetailEntry) -> bool` (autorización vacía) para LINK/CLIENT.

## 3. Reglas

- PROTO-08: Integrado en su propio módulo; Autoservicio lo hace AUTO-01.
- Sin `f32`/`f64` (API-03). Sin pánicos.
- Fixtures: `crates/transbank-pos/tests/fixtures/integrado.txt` con **todas** las tramas de ejemplo de protocol-spec §5 (una por línea, comentario `# origen: transbank-pos-sdk-dotnet tests`).

## 4. Tests

- Un test por payload de request contra el string exacto (ej. `sale_payload(&SaleRequest::new(12000,"ABC123")?, false) == "0200|000012000|ABC123||0|0"`; ticket `"42"` ⇒ `000042`; `refund_payload(142) == "1200|000142|"`; `sales_detail_payload(SendToHost) == "0260|1|"`, `PrintOnPos ⇒ "0260|0|"`).
- Validaciones: monto 49 y 1_000_000_000 rechazados; 50 y 999_999_999 aceptados; ticket vacío, de 7, con `-` o `ñ` rechazados.
- Cada fixture parsea con los valores de las tablas de §5 (verificar **todos** los campos en al menos el ejemplo de `0210`, `0261`, `0271`, `0291`, `0710`, `1210`).
- Rechazo `0271|07|||ABC123||90000|…` ⇒ `Ok`, `is_approved() == false`, `response_code == CanceledAtPos` (o el nombre elegido para 7).
- Campo numérico inválido (`amount = "12a00"`) ⇒ `Error::Parse { field: "amount", .. }`.
- Fecha `00-00-00` ⇒ `None`. `last4_digits = "0331"` conserva el cero.
- Código de función equivocado ⇒ `UnexpectedResponse`.
- `format!("{:?}", resp)` no contiene el `account_number` ni el voucher.
- `proptest`: ningún payload arbitrario hace pánico en ningún parser.

## 5. Definición de terminado

- [ ] RF-01…RF-05.
- [ ] AGENTS.md §8 en verde.
- [ ] Reporte completo (lista de nombres de variantes de `ResponseCode`).

## Reporte

_(pendiente)_
