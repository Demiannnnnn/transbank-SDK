# Tarea SIM-01: Simulador de POS (sans-IO, escenarios, `SimTransport`, bin TCP)

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-SIM-01` |
| **Estado** | ⬜ pendiente |
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

- [ ] RF-01…RF-05.
- [ ] AGENTS.md §8 en verde.
- [ ] Prueba manual: `cargo run -p transbank-pos-sim -- --listen 127.0.0.1:7777` y un `nc`/script envía `0100`; resultado en el Reporte.
- [ ] Reporte completo.

## Reporte

_(pendiente)_
