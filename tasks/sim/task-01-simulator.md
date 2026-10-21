# Tarea SIM-01: Simulador de POS (sans-IO, escenarios, `SimTransport`, bin TCP)

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-SIM-01` |
| **Estado** | ✅ hecho |
| **Rama** | `dev/sim-01-simulator` |
| **Depende de** | PROTO-02, LINK-01 |
| **Restricciones** | PROTO-02, SEC-04, LEG-03, QA-03, SAFE-02 · `docs/simulator-spec.md` completo · `docs/protocol-spec.md` §3–§6 |
| **Prioridad** | P0 |

## 1. Contexto

Un POS falso para probar el cliente sin hardware (QA-02) y para que los usuarios del SDK prueben sus apps. Implementa `docs/simulator-spec.md`. Integrado completo; Autoservicio lo agrega AUTO-01 (deja `TerminalKind::Autoservicio` respondiendo ACK a `0100` y código 12 "Transacción No Soportada" al resto, documentado como pendiente).

## 2. Requerimientos

### RF-01 `PosSimulator` (lib, sans-IO)
API de simulator-spec §1; reutiliza `transbank_pos::protocol::{FrameDecoder, encode_frame, lrc}` y los parsers/payloads de `message` cuando aplique (para leer requests escribe un parser propio de requests del lado POS en el crate sim: `parse_request(payload) -> Result<SimRequest>` que acepta las variantes de OPEN-01).
Enlace del lado POS (§1): ACK/NAK a lo recibido, espera de ACK de su respuesta y retransmisión ante NAK (máx. 3).

### RF-02 Escenarios
Todos los de simulator-spec §2, con consumo de los "próximos N".

### RF-03 Reglas de negocio Integrado
Tabla de simulator-spec §3, con fecha/hora fija configurable y autorización determinista.

### RF-04 `SimTransport`
simulator-spec §4: implementa `transbank_pos::Transport` sobre un `PosSimulator` y un `ManualClock`. `sim_mut()`.

### RF-05 `TcpServer` + bin `tbk-pos-sim`
- `TcpServer::bind(addr, allow_remote: bool) -> Result<Self>`: rechaza direcciones no loopback sin `allow_remote` (SEC-04). `local_addr()`. `serve_one(&mut self, sim: &mut PosSimulator) -> Result<()>` atiende una conexión hasta que el cliente cierra. `run(self, sim)` en bucle.
- Bin con los flags de simulator-spec §5 (`clap`), salida en español con `<-`/`->`, `--quiet`, aviso no oficial en `--help` (LEG-03). Sin `ctrlc` crate: terminar con la señal por defecto es aceptable (documenta).

## 3. Reglas

- El simulador nunca usa `rand` ni la hora del sistema en la lógica (determinista).
- Sin pánicos en la lib (mismas reglas que la librería).

## 4. Tests

- `feed` con la trama de venta ⇒ ACK + respuesta `0210` con LRC correcto; sin ACK del host y con NAK ⇒ retransmite.
- Cada fila de simulator-spec §3 (incluye anulación de débito ⇒ 8, doble anulación ⇒ 4, detalle con 0 y 3 ventas, cierre borra ventas, `RequireClose`).
- Cada escenario de §2 produce lo especificado (bytes).
- Acepta las tres variantes de venta de OPEN-01.
- `SimTransport` + `Link` (de LINK-01): `request` de venta aprobada end-to-end.
- `TcpServer` en `127.0.0.1:0`: un `TcpStream` de prueba envía `0100` y recibe ACK; `bind("0.0.0.0:0", false)` ⇒ error.
- El bin: `tbk-pos-sim --help` contiene "no oficial".

## 5. Definición de terminado

- [x] RF-01…RF-05.
- [x] AGENTS.md §8 en verde.
- [x] Prueba manual: `cargo run -p transbank-pos-sim -- --listen 127.0.0.1:7777` y un `nc`/script envía `0100`; resultado en el Reporte.
- [x] Reporte completo.

## Reporte

Implementada por Claude (fallback: cuota Gemini en 0 %), 2026-10-21.

- **Crate `transbank-pos-sim`:** `PosSimulator`, `TerminalKind`, `Scenario`, `SimulatedSale`, `SimTransport`, `TcpServer`, `parse_request`/`SimRequest`/`SaleArgs` y las constantes `SIM_COMMERCE_CODE`/`SIM_TERMINAL_ID`. Reutiliza `FrameDecoder` y `encode_frame` de `transbank_pos::protocol`, así que el LRC tiene una sola implementación.
- **Enlace del lado POS:** responde ACK o NAK a lo que recibe. Tras enviar una trama, espera el ACK del host antes de mandar la siguiente (`0900` y detalle); ante un NAK retransmite hasta 3 veces y luego desiste. Un comando nuevo reemplaza lo que estuviera pendiente.
- **Escenarios, con estas interpretaciones:**
  - `Intermediate(codes)` solo se aplica si el host pidió `status = 1`, como hace un POS real. Si el host lo pide sin el escenario, se envían 78, 79, 81 y 82.
  - `DropAcks` y `NakNext` no procesan el comando.
  - `CorruptLrc(n)` cuenta transmisiones, incluidas las retransmisiones.
  - `WrongFunctionCode` cambia la primera trama que no es `0900`.
  - `NoResponse` y `WithVoucher` son persistentes.
  - Sin `WithVoucher`, el voucher solo se incluye si el host lo pidió (3 líneas).
- **Reglas de negocio (§3):**
  - Monto < 50 ⇒ 10; > 999 999 999 ⇒ 68; ticket > 6 ⇒ 70; `RequireClose` ⇒ 74.
  - Monto par ⇒ crédito `CR`/`VI` (3 cuotas desde 10 000); impar ⇒ débito `DB`/`DB`.
  - Autorización determinista: `(op × 7919 + 104729) mod 10⁶`. Fecha y hora fijas `01012026`/`120000`, configurables con `with_clock_fixed`.
  - La anulación aprobada consume un número de operación.
  - Una request mal formada responde código 61.
  - Autoservicio queda pendiente: ACK a `0100` y `{fc+10}|12` al resto.
- **Bin `tbk-pos-sim`:**
  - Flags de §5 más los escenarios `drop-acks:N`, `nak:N` y `voucher:N`; `--quiet`.
  - Muestra `<-`/`->` con ACK/NAK; `--help` dice "Simulador NO oficial".
  - Ctrl-C usa el manejo de señales por defecto: no se agregó ningún crate.
  - `TcpServer::bind` rechaza direcciones que no son loopback sin `allow_remote` (SEC-04). `with_delay` usa `thread::sleep` solo en el servidor, nunca en tests.
  - `serve_one_with(sim, observer)` existe para que el bin muestre lo intercambiado.
- **Tests:** 31 de integración en `tests/simulator.rs` (cada fila de §3, cada escenario, las tres variantes de OPEN-01, `SimTransport` con el `Link` real, incluida una venta colgada que no se reenvía, TCP en `127.0.0.1:0` con hilo y `join`, rechazo de `0.0.0.0` y el `--help` del bin), más unitarios y doctests. 152 tests en total en el workspace.
- **Prueba manual:** `tbk-pos-sim --listen 127.0.0.1:7777` con un cliente Python por TCP.
  - `0100` ⇒ `06`.
  - Venta `0200|000012000|ABC123||0|0` ⇒ `06` y luego `0210|00|597029414300|SIM00001|ABC123|112648|12000|03|4000|0037|000001|CR|01012026|0000000000000000037|VI|01012026|120000||0|`.
  - La consola mostró `<- 0100`, `-> ACK`, `<- 0200|…`, `-> ACK`, `-> 0210|…`, `<- ACK` y `cliente desconectado`.
- AGENTS.md §8: los cinco comandos en verde; `validate.sh` OK.
