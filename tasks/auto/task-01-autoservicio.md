# Tarea AUTO-01: POS Autoservicio (mensajes, cliente, simulador y CLI)

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-AUTO-01` |
| **Estado** | ⬜ pendiente |
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

_(pendiente)_
