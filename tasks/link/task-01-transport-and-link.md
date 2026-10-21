# Tarea LINK-01: `Transport`, `Clock`, `MemoryTransport` y capa de enlace

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-LINK-01` |
| **Estado** | ⬜ pendiente |
| **Rama** | `dev/link-01-transport-and-link` |
| **Depende de** | PROTO-02 (usa `IntermediateStatus`, `parse_intermediate` y los `FC_*`) |
| **Restricciones** | LINK-01…07, SAFE-02, SAFE-03, SEC-02, QA-03, ADR-04, ADR-08, ADR-09 · `docs/protocol-spec.md` §3, §4.2, §8 · `public-api.md` §6–§7 · `security-model.md` §3 |
| **Prioridad** | P0 |

## 1. Contexto

El enlace es el único lugar con tiempo y reintentos. Debe ser correcto ante POS lentos, NAK, LRC corrupto, basura y cortes, **sin reenviar nunca una venta ya acusada**. Todo se prueba con tiempo simulado.

## 2. Requerimientos

### RF-01 `transport/mod.rs`
Trait `Transport` (con `discard_input` por defecto) e impls para `&mut T` y `Box<T>`; trait `Clock`, `SystemClock` (`Instant` interno), según `public-api.md` §6.

### RF-02 `transport/memory.rs`
`ManualClock` (Clone, tiempo compartido con `Arc<Mutex<Duration>>` o `Arc<AtomicU64>` en nanos; `advance`, `now`) y `MemoryTransport` con guion (`public-api.md` §6.1). `read_timeout` sin datos avanza el reloj `timeout` y devuelve `Ok(0)`; con datos, entrega hasta `buf.len()` bytes. Escritura fuera de guion ⇒ `Error::Io(InvalidData)` con hex esperado y recibido. Útil también para usuarios: documentado con ejemplo.

### RF-03 `link.rs`
`LinkConfig` (defaults de LINK-04) y `Link<T, C>` con `send_command`, `request`, `request_list` (`public-api.md` §7):
- Antes de enviar: `discard_input()` y `FrameDecoder::reset()`.
- Envío con espera de ACK y reintentos (§3.1, LINK-02). `NoAck { attempts, naks }`.
- Recepción (§3.2): ACK inmediato a cada trama válida (LINK-01), NAK a `BadLrc` (máx. `max_bad_lrc` seguidos), `0900` al callback (también con ACK, OPEN-02), código inesperado ⇒ `UnexpectedResponse` (LINK-06), basura ignorada (LINK-07), plazo total `response_timeout` sin reinicio (LINK-04).
- `request_list` (§4.2): acumula tramas `expected`, detecta fin por autorización vacía (campo 5 tras `split('|')`, trim) y luego consume tramas vacías adicionales hasta `detail_tail` sin bytes; límite 10 000 (`TooManyFrames`, SAFE-02). Devuelve los payloads de las ventas (sin las vacías); si la primera vacía trae código `11` ⇒ vec vacío.
- Un `read_timeout` nunca recibe un plazo mayor al restante (cálculo con `Clock`).
- **No existe** en el enlace ningún camino que reescriba la trama tras recibir su ACK (LINK-03).
- Feature `tracing`: eventos de `security-model.md` §3 (redacción mínima: en `trace` solo código de función y largo hasta que exista el redactor por layout; PROTO-02 provee el layout — si ya está integrado, redacta `account_number`/voucher).

## 3. Reglas

- Sin `thread::sleep`, sin hilos.
- Nada específico de serie/TCP en `link.rs`.

## 4. Tests (`MemoryTransport` + `ManualClock`)

- `send_command("0100")`: escribe `02 30 31 30 30 03 02`, recibe ACK ⇒ `Ok`.
- Sin ACK ⇒ 3 envíos idénticos y `NoAck { attempts: 3, naks: 0 }`; con NAK, NAK, ACK ⇒ `Ok` tras 3 envíos; NAK×3 ⇒ `NoAck { attempts: 3, naks: 3 }`. El reloj avanzó `ack_timeout` por cada espera vencida.
- `request` venta: ACK, `0900|78`, `0900|81`, `0210|00|…` ⇒ callback recibe 78 y 81 en orden; el host escribió ACK tras cada una de las tres tramas.
- Respuesta con LRC malo luego buena ⇒ NAK y luego ACK, `Ok`. LRC malo ×4 ⇒ `BadLrc { count: 3 }` (o el número de la regla; documenta).
- Basura antes del ACK y entre tramas ⇒ `Ok`.
- Respuesta `0810` cuando se esperaba `0210` ⇒ `UnexpectedResponse`.
- **LINK-03:** venta con ACK y sin respuesta ⇒ `ResponseTimeout` tras `response_timeout` de reloj simulado y la trama de venta se escribió **exactamente una vez**.
- `0900` repetidos no extienden el plazo: con mensajes cada 100 s, timeout a los 150 s.
- `request_list`: 3 ventas + 2 vacías ⇒ 3 payloads; 3 ventas + 1 vacía ⇒ 3 (espera `detail_tail`); `0261|11|…` ⇒ vacío; 10 001 tramas ⇒ `TooManyFrames`.
- `Link<MemoryTransport>` es `Send` (API-07).
- Suite completa en < 1 s reales.

## 5. Definición de terminado

- [ ] RF-01…RF-03.
- [ ] AGENTS.md §8 en verde.
- [ ] Reporte completo.

## Reporte

_(pendiente)_
