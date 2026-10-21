# Tarea SERIAL-01: `SerialTransport`, `list_ports`, `autoconnect` y `TcpTransport`

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-SERIAL-01` |
| **Estado** | ⬜ pendiente |
| **Rama** | `dev/serial-01-serial-and-tcp` |
| **Depende de** | LINK-01, CLIENT-01 |
| **Restricciones** | API-07, API-08, SAFE-03, QA-03 · `public-api.md` §6, §8 · `docs/protocol-spec.md` §1 |
| **Prioridad** | P1 |

## 1. Contexto

Transportes reales. El serie no se puede probar con hardware en esta tarea (AGENTS.md §7): se prueba la lógica que no depende del puerto (conversión de errores, orden de autodetección) y se deja la validación a HW-01. TCP sí se prueba de punta a punta contra el simulador.

## 2. Requerimientos

### RF-01 `transport/tcp.rs`
`TcpTransport::connect(addr, connect_timeout) -> Result<Self>`; `TCP_NODELAY`; `read_timeout` con `set_read_timeout` (un `timeout` cero se trata como 1 ms; `WouldBlock`/`TimedOut` ⇒ `Ok(0)`; EOF ⇒ `Error::Io(UnexpectedEof)`); `discard_input` lee sin bloquear lo pendiente.

### RF-02 `serial.rs` (feature `serial`)
- `SerialTransport::open(path, baud_rate)`: 8N1, sin control de flujo, `DataBits::Eight`, timeout de lectura configurado por llamada. Errores de `serialport` ⇒ `Error::Transport` (o `Io` si son de E/S). `discard_input` con `clear(ClearBuffer::Input)`.
- Baudios aceptados: los de protocol-spec §1; otro ⇒ `InvalidArgument`.
- `list_ports()` ⇒ `PortInfo` (USB con VID/PID/producto). En macOS, si existen `cu.*` y `tty.*` del mismo dispositivo, lista solo `cu.*`.
- Constantes de baudios por defecto.

### RF-03 `PosIntegrado::open` y `autoconnect`
`autoconnect(baud)`: ordena los puertos (USB primero, luego el resto, orden estable por nombre), abre cada uno, hace `poll` con `ack_timeout` por defecto y devuelve el primero que responde; puertos que fallan se cierran (drop) y se sigue. La lógica de orden va en una función pura `order_candidates(Vec<PortInfo>) -> Vec<PortInfo>` testeable.

### RF-04 Ejemplos
Los ejemplos de CLIENT-01 pasan a `TcpTransport` (variable `TBK_POS_TCP`) o `SerialTransport` (`TBK_POS_PORT`).

## 3. Reglas

- Ningún test abre un puerto serie real (QA-03). Con la feature `serial` desactivada todo compila (API-08).

## 4. Tests

- TCP end-to-end: `TcpServer` del simulador en `127.0.0.1:0` en un hilo **con `join`** y `PosIntegrado<TcpTransport>` hace `poll`, `sale`, `last_sale`, `sales_detail`.
- TCP: servidor que cierra la conexión ⇒ `Io(UnexpectedEof)`; servidor que no responde ⇒ `NoAck` en tiempo real acotado (usa `LinkConfig` con `ack_timeout` de 50 ms para que el test dure poco).
- `order_candidates`: USB primero, estable; `cu.*` vs `tty.*` en macOS.
- Baudios inválidos ⇒ `InvalidArgument` (sin abrir nada: valida antes).
- `SerialTransport: Send`, `TcpTransport: Send`.

## 5. Definición de terminado

- [ ] RF-01…RF-04.
- [ ] AGENTS.md §8 en verde.
- [ ] Reporte con lo que solo se pudo verificar en macOS/Linux y lo que queda para HW-01.

## Reporte

_(pendiente)_
