# Tarea SERIAL-01: `SerialTransport`, `list_ports`, `autoconnect` y `TcpTransport`

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-SERIAL-01` |
| **Estado** | ✅ hecho |
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

- [x] RF-01…RF-04.
- [x] AGENTS.md §8 en verde.
- [x] Reporte con lo que solo se pudo verificar en macOS/Linux y lo que queda para HW-01.

## Reporte

Implementada por Claude (fallback: cuota Gemini en 0 %), 2026-10-21.

- **`transport/tcp.rs`, `TcpTransport`:**
  - `connect` prueba cada dirección resuelta con `connect_timeout`. `from_stream` es un agregado, útil para puentes.
  - `TCP_NODELAY`.
  - `read_timeout`: un timeout cero se trata como 1 ms. `WouldBlock`, `TimedOut` e `Interrupted` ⇒ `Ok(0)`; EOF ⇒ `Io(UnexpectedEof)`.
  - `discard_input` lee sin bloquear lo que haya pendiente.
- **`serial.rs`, `SerialTransport`:**
  - 8N1, sin control de flujo, timeout por llamada (mínimo 1 ms).
  - Errores: `serialport::ErrorKind::Io` ⇒ `Error::Io`; el resto ⇒ `Error::Transport`.
  - `discard_input` usa `clear(Input)`; el puerto se cierra en `drop`.
  - Baudios permitidos: la constante pública agregada `SUPPORTED_BAUD_RATES` (1200…115200). Se validan **antes** de abrir nada.
  - `list_ports()` ⇒ `PortInfo`; `PortKind` es `#[non_exhaustive]`. En todos los sistemas se quita el `/dev/tty.X` cuando existe `/dev/cu.X`.
  - `order_candidates` es pública y pura: USB primero, luego el resto, cada grupo ordenado por nombre.
- **`PosIntegrado::open(path)` y `PosIntegrado::autoconnect(baud)`:** valida los baudios, recorre `order_candidates(list_ports()?)` y salta los puertos que no abren o no responden al `poll` (se cierran en `drop`).
- **Ejemplos:** `poll`, `sale` y `detail` se conectan a `TBK_POS_PORT` (serie, con `cfg(feature = "serial")`) o a `TBK_POS_TCP` (por defecto `127.0.0.1:7777`) con `Box<dyn Transport>`. Ya no usan el simulador en memoria, así que el `.crate` no depende de él por los ejemplos. `detail` ya no hace ventas, solo lee. Probados contra `tbk-pos-sim`: `poll` ⇒ "POS conectado"; `sale` muestra los 4 mensajes `0900` y "Aprobada: $12.000…"; `detail` muestra 1 venta y los totales.
- **Tests:**
  - `tests/transport_tcp.rs`, todo en `127.0.0.1:0` y con `join`:
    - `poll`, `sale`, `last_sale` y `sales_detail` contra `TcpServer`;
    - servidor que cierra ⇒ `Io(UnexpectedEof)`;
    - servidor mudo ⇒ `NoAck { 3, 0 }` en menos de 2 s reales (`ack_timeout` de 50 ms; el servidor recibió exactamente 21 bytes = 3 × `0100`);
    - `read_timeout(0)` ⇒ `Ok(0)`.
  - Unitarios de `serial`: orden, duplicados de macOS, baudios inválidos en `open` y `autoconnect` sin abrir nada, mapeo de errores.
  - `Send` de ambos transportes y de `PosIntegrado` sobre ellos.
  - 185 tests en total en el workspace.
- **Verificado solo en macOS:**
  - compilación y tests con y sin `serial` (`clippy --no-default-features --all-targets` en verde);
  - `list_ports` compila, pero no se ejecuta en tests para no depender del hardware.
  - Linux y Windows dependen del CI manual.
- **Pendiente para HW-01:**
  - abrir un POS real por USB;
  - que el timeout de `serialport` respete plazos cortos en cada sistema operativo;
  - que `clear(Input)` no pierda la respuesta;
  - si llega basura al abrir (OPEN-05);
  - `autoconnect` con varios puertos.
- AGENTS.md §8: los cinco comandos en verde; `validate.sh` OK.
