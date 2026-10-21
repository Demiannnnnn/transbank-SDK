# Tarea PROTO-01: LRC, codificador y decodificador de tramas

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-PROTO-01` |
| **Estado** | ⬜ pendiente |
| **Rama** | `dev/proto-01-framing` |
| **Depende de** | INFRA-01 |
| **Restricciones** | PROTO-01…04, SAFE-02, API-05, API-06, ADR-05, ADR-06 · `docs/protocol-spec.md` §2 |
| **Prioridad** | P0 |

## 1. Contexto

Base de todo el SDK: convertir payloads en tramas `STX payload ETX LRC` y reconocer tramas, ACK y NAK en un flujo de bytes que llega en trozos arbitrarios. Lee `docs/protocol-spec.md` §2 completo, en especial §2.3 (vectores) y §2.4 (máquina de estados).

## 2. Requerimientos

### RF-01 `error.rs`
`Error` y `Result` completos según `public-api.md` §1 (todas las variantes, aunque algunas se usen recién en LINK-01).

### RF-02 `protocol/mod.rs` — constantes
`STX`, `ETX`, `ACK`, `NAK`, `FIELD_SEPARATOR` (`b'|'`), `MAX_PAYLOAD = 4096`. Públicas en `transbank_pos::protocol`.

### RF-03 `protocol/lrc.rs`
`pub fn lrc(payload: &[u8]) -> u8` — XOR del payload y de `ETX` (PROTO-02). Documentar con el vector `0100` ⇒ `0x02` como doctest.

### RF-04 `protocol/frame.rs`
- `pub fn encode_frame(payload: &str) -> Result<Vec<u8>>`: valida que el payload no esté vacío, sea ASCII imprimible (`0x20..=0x7E`) y mida ≤ `MAX_PAYLOAD`; si no ⇒ `Error::InvalidArgument { field: "payload", … }` u `Overflow`.
- `pub fn decode_latin1(bytes: &[u8]) -> String` (ADR-06).

### RF-05 `protocol/decoder.rs`
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LinkEvent {
    Ack, Nak,
    Frame(Vec<u8>),                                   // payload sin STX/ETX/LRC
    BadLrc { payload: Vec<u8>, expected: u8, received: u8 },
    Garbage(usize),                                   // bytes descartados fuera de trama
    Overflow,
}
#[derive(Debug, Default)]
pub struct FrameDecoder { … }
impl FrameDecoder {
    pub fn new() -> Self;
    /// Procesa un trozo y agrega los eventos a `out` (sin asignar si no hay eventos).
    pub fn push(&mut self, bytes: &[u8], out: &mut Vec<LinkEvent>);
    pub fn is_idle(&self) -> bool;                     // no hay trama a medias
    pub fn reset(&mut self);
}
```
Máquina de estados **exacta** de protocol-spec §2.4. `Garbage(n)` agrupa bytes consecutivos descartados y se emite al volver a ver un byte significativo (o al final del trozo si se está en `Idle`; documenta cuál eliges y que sea consistente para cualquier partición — ver test de partición).

## 3. Reglas

- Sin E/S, sin tiempo, sin `std::io` en `protocol`.
- Sin indexación con pánico; sin `unwrap`/`expect` fuera de tests.
- Los tipos públicos documentados con ejemplo (API-06).

## 4. Tests

- Los 9 vectores de protocol-spec §2.3: `encode_frame` produce exactamente esos bytes y el decodificador los devuelve como `Frame`.
- `0100` (LRC = STX) y `0500||` (LRC = ACK) seguidos en el mismo trozo ⇒ dos `Frame` (PROTO-03).
- LRC alterado ⇒ `BadLrc` con `expected` y `received` correctos.
- `ACK`, `NAK` sueltos; basura antes de una trama ⇒ `Garbage(n)` + `Frame`; `STX` dentro de una trama ⇒ reinicio; payload de 4097 bytes ⇒ `Overflow` y luego decodifica bien la siguiente.
- `encode_frame` rechaza vacío, `\x02`, `\x03`, `ñ`, `\n`, 4097 bytes.
- `proptest` (≥ 256 casos cada uno):
  - `decode(encode(p)) == p` para payloads ASCII imprimibles de 1..=200.
  - Una secuencia de tramas/ACK/NAK partida en trozos aleatorios produce los mismos eventos que entregada entera (los `Garbage` deben sumar lo mismo).
  - Bytes arbitrarios (0..2000) nunca hacen pánico.

## 5. Definición de terminado

- [ ] RF-01…RF-05.
- [ ] AGENTS.md §8 en verde.
- [ ] Reporte completo (incluye el número de tests y la decisión sobre `Garbage`).

## Reporte

_(pendiente)_
