# API pública — `transbank-pos` (contrato)

Contrato con los usuarios del crate (API-01). Las firmas son normativas en nombres, parámetros, tipos de retorno y semántica; los detalles internos (campos privados, módulos internos) son libres. Cambiar algo de aquí requiere que la tarea lo diga. Versión objetivo: **0.1.0**.

Re-exports en la raíz (`transbank_pos::`): todo lo marcado **[raíz]**.

## 1. Errores — `error.rs`

```rust
/// [raíz]
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// [raíz] Falla de comunicación o de protocolo. Un rechazo de la tarjeta NO es un Error (ADR-03).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Error del transporte (p. ej. serialport) que no es io::Error.
    #[error("transport error: {0}")]
    Transport(Box<dyn std::error::Error + Send + Sync>),
    /// La trama no fue acusada tras `attempts` envíos (`naks` de ellos respondidos con NAK).
    #[error("POS did not acknowledge the command after {attempts} attempts ({naks} NAK)")]
    NoAck { attempts: u8, naks: u8 },
    /// No llegó respuesta en `response_timeout`. En una venta el resultado es DESCONOCIDO: usar last_sale().
    #[error("no response from POS within {waited:?}")]
    ResponseTimeout { waited: std::time::Duration },
    /// Demasiadas tramas con LRC inválido seguidas.
    #[error("received {count} frames with invalid LRC")]
    BadLrc { count: u8 },
    #[error("expected response {expected}, received {received}")]
    UnexpectedResponse { expected: &'static str, received: String, payload: String },
    #[error("invalid value {value:?} in field {field}")]
    Parse { field: &'static str, value: String, payload: String },
    #[error("invalid argument {field}: {reason}")]
    InvalidArgument { field: &'static str, reason: &'static str },
    #[error("frame larger than {max} bytes")]
    Overflow { max: usize },
    #[error("more than {max} frames in a multi-frame response")]
    TooManyFrames { max: usize },
}
```

`Error` es `Send + Sync + 'static`.

## 2. Tipos de dominio — `protocol/types.rs`

```rust
/// [raíz] Monto en CLP. Venta: 50..=999_999_999.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Amount(u32);
impl Amount {
    pub const MIN_SALE: u32 = 50;
    pub const MAX: u32 = 999_999_999;
    pub fn new(clp: u32) -> Result<Self>;        // InvalidArgument fuera de rango
    pub fn get(self) -> u32;
}
impl TryFrom<u32> for Amount { type Error = Error; }

/// [raíz] Número de ticket/boleta: 1..=6 ASCII alfanuméricos. Se envía con '0' a la izquierda hasta 6.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ticket(String);
impl Ticket { pub fn new(s: &str) -> Result<Self>; pub fn as_str(&self) -> &str; pub fn padded(&self) -> String; }
impl TryFrom<&str> for Ticket { type Error = Error; }

/// [raíz] Número de operación para anular: 1..=999_999 (se envía con 6 dígitos).
pub struct OperationNumber(u32);  // new(u32) -> Result<Self>, get(), TryFrom<u32>, FromStr

/// [raíz] Código de comercio (multicódigo): "0" o 1..=12 dígitos.
pub struct CommerceCode(String);  // new(&str) -> Result<Self>, as_str(), principal() -> Self ("0")

/// [raíz] Fecha/hora tal como las informa el POS (sin zona horaria).
pub struct PosDate { pub day: u8, pub month: u8, pub year: u16 }   // parse("ddMMyyyy") -> Option<Self>
pub struct PosTime { pub hour: u8, pub minute: u8, pub second: u8 } // parse("HHmmss") -> Option<Self>

/// [raíz]
#[non_exhaustive] pub enum CardType { Credit, Debit, Prepaid, Other(String) }      // "CR" "DB" "PR"
/// [raíz]
#[non_exhaustive] pub enum CardBrand { Visa, Mastercard, Amex, Redcompra, Diners, Magna, Other(String) } // VI MC AX DB DC MG
```

Todos derivan `Debug, Clone, PartialEq, Eq` (y `Copy`/`Hash` donde aplique); con la feature `serde`, `Serialize` (y `Deserialize` en los de entrada). `Display` en `Amount` (`$12.000`), `Ticket`, `PosDate` (`dd-mm-aaaa`), `PosTime`.

## 3. Códigos — `codes.rs`

```rust
/// [raíz] Código de respuesta (protocol-spec §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResponseCode {
    Approved,                 // 0
    Rejected,                 // 1
    HostNotResponding,        // 2
    // … una variante por cada código de §9 (nombres en inglés, descriptivos) …
    InitializationOk,         // 90 (Autoservicio)
    Other(u16),
}
impl ResponseCode {
    pub fn from_code(code: u16) -> Self;
    pub fn code(self) -> u16;
    pub fn message(self) -> &'static str;   // mensaje en español de §9; Other ⇒ "Código desconocido"
    pub fn is_approved(self) -> bool;        // Approved | InitializationOk
    pub fn requires_close(self) -> bool;     // 13, 67, 74, 76, 77
    pub fn requires_load_keys(self) -> bool; // 26
}
impl std::fmt::Display for ResponseCode { /* "{code:02} {message}" */ }

/// [raíz] Mensaje intermedio 0900.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IntermediateStatus { Code(ResponseCode), Unknown(String) }
impl IntermediateStatus { pub fn message(&self) -> &str; }
```

## 4. Requests — `message/`

```rust
/// [raíz]
pub struct SaleRequest { /* amount: Amount, ticket: Ticket, voucher: bool */ }
impl SaleRequest {
    pub fn new(amount_clp: u32, ticket: &str) -> Result<Self>;
    pub fn from_parts(amount: Amount, ticket: Ticket) -> Self;
    pub fn with_voucher(self, voucher: bool) -> Self;   // pedir el voucher en la respuesta
    pub fn amount(&self) -> Amount; pub fn ticket(&self) -> &Ticket; pub fn voucher(&self) -> bool;
}
/// [raíz]
pub struct MultiCodeSaleRequest { /* SaleRequest + commerce_code: CommerceCode */ }
impl MultiCodeSaleRequest { pub fn new(amount_clp: u32, ticket: &str, commerce_code: &str) -> Result<Self>; pub fn with_voucher(self, v: bool) -> Self; }

/// [raíz]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SalesDetailMode { PrintOnPos, SendToHost }
```

El módulo `message` expone además (público, para usuarios avanzados y el simulador):

```rust
pub mod message {
    /// Payload de cada request, exactamente como protocol-spec §4/§7.
    pub fn sale_payload(req: &SaleRequest, status: bool) -> String;   // y uno por comando
    /// Parsers: fn parse_<respuesta>(payload: &str) -> Result<…Response>
}
```

## 5. Respuestas — `message/responses.rs`

Structs `#[non_exhaustive]`, campos públicos, `Debug` que redacta (`SEC-03`), `Clone`, `PartialEq`; `serde::Serialize` con la feature.

```rust
/// [raíz] 0210 / 0260 (Integrado).
pub struct SaleResponse {
    pub function_code: String,
    pub response_code: ResponseCode,
    pub commerce_code: Option<String>,
    pub terminal_id: Option<String>,
    pub ticket: Option<String>,
    pub authorization_code: Option<String>,
    pub amount: Option<u32>,
    pub installments_number: Option<u32>,
    pub installments_amount: Option<u32>,
    pub last4_digits: Option<String>,
    pub operation_number: Option<String>,
    pub card_type: Option<CardType>,
    pub accounting_date: Option<PosDate>,
    pub account_number: Option<String>,
    pub card_brand: Option<CardBrand>,
    pub real_date: Option<PosDate>,
    pub real_time: Option<PosTime>,
    pub employee_id: Option<u32>,
    pub tip: Option<u32>,
    pub voucher: Option<String>,
    pub raw: String,
}
impl SaleResponse {
    pub fn is_approved(&self) -> bool;
    pub fn voucher_lines(&self) -> Option<Vec<&str>>;  // líneas de 40 si el largo es múltiplo de 40
}
pub struct MultiCodeSaleResponse { pub sale: SaleResponse, pub change: Option<u32>, pub commerce_provider_code: Option<String> } // 0271/0281
pub struct DetailEntry { /* campos §5.3 + raw */ }                     // 0261
pub struct MultiCodeDetailEntry { pub sale: SaleResponse, pub change: Option<u32>, pub commerce_provider_code: Option<String> } // 0291
pub struct LoadKeysResponse { pub response_code, pub commerce_code, pub terminal_id, pub raw }   // 0810
pub struct CloseResponse { pub response_code, pub commerce_code, pub terminal_id, pub voucher: Option<String>, pub raw } // 0510
pub struct TotalsResponse { pub response_code, pub tx_count: Option<u32>, pub tx_total: Option<u64>, pub raw }          // 0710
pub struct RefundResponse { pub response_code, pub commerce_code, pub terminal_id, pub authorization_code, pub operation_number, pub raw } // 1210
// Autoservicio (AUTO-01): SelfServiceSaleResponse (layout §7.1), InitializationResponse (1080)
```

Todas tienen `is_approved()`.

## 6. Transporte y reloj — `transport/`, `link.rs`

```rust
/// [raíz] Canal de bytes con el POS.
pub trait Transport: Send {
    fn write_all(&mut self, bytes: &[u8]) -> Result<()>;
    /// Espera hasta `timeout` a que haya bytes; Ok(0) = venció sin datos. Nunca bloquea más que `timeout`.
    fn read_timeout(&mut self, buf: &mut [u8], timeout: std::time::Duration) -> Result<usize>;
    /// Descarta la entrada pendiente (antes de enviar un comando). Por defecto no hace nada.
    fn discard_input(&mut self) -> Result<()> { Ok(()) }
}
impl<T: Transport + ?Sized> Transport for &mut T { … }
impl<T: Transport + ?Sized> Transport for Box<T> { … }

/// [raíz] Reloj monotónico inyectable (ADR-09).
pub trait Clock: Send { fn now(&self) -> std::time::Duration; }
/// [raíz]
#[derive(Debug, Clone, Copy, Default)] pub struct SystemClock;   // Instant desde la creación

pub mod transport {
    pub struct MemoryTransport;   // §6.1
    pub struct ManualClock;       // Clone, comparte el tiempo: advance(d), now()
    pub struct TcpTransport;      // connect(addr: impl ToSocketAddrs, connect_timeout) -> Result<Self>
}
#[cfg(feature = "serial")]
pub mod serial {
    pub struct SerialTransport;   // open(path: &str, baud_rate: u32) -> Result<Self>; 8N1, sin control de flujo
    pub struct PortInfo { pub name: String, pub kind: PortKind, pub vid: Option<u16>, pub pid: Option<u16>, pub product: Option<String> }
    pub enum PortKind { Usb, Pci, Bluetooth, Unknown }
    pub fn list_ports() -> Result<Vec<PortInfo>>;
    pub const DEFAULT_BAUD_INTEGRADO: u32 = 115_200;
    pub const DEFAULT_BAUD_AUTOSERVICIO: u32 = 19_200;
}
```

### 6.1 `MemoryTransport` (para tests de usuarios y del SDK)

Transporte en memoria con **guion**: `MemoryTransport::new(clock: ManualClock)`; `expect_write(bytes)`, `then_reply(bytes)` encadenables; `push_incoming(bytes)`; `written() -> &[u8]`. `read_timeout` sin datos pendientes **avanza el `ManualClock` en `timeout`** y devuelve `Ok(0)`, así los timeouts se prueban en milisegundos reales. Una escritura que no coincide con el guion devuelve `Error::Io` (`InvalidData`) con ambos valores en hex.

## 7. Enlace — `link.rs`

```rust
/// [raíz]
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LinkConfig {
    pub ack_timeout: Duration,       // 2 s
    pub response_timeout: Duration,  // 150 s
    pub detail_tail: Duration,       // 500 ms
    pub max_attempts: u8,            // 3
    pub max_bad_lrc: u8,             // 3
}
impl Default for LinkConfig { … }

/// [raíz] Enlace de bajo nivel (usuarios avanzados; los clientes lo usan por dentro).
pub struct Link<T: Transport, C: Clock = SystemClock> { … }
impl<T: Transport> Link<T> { pub fn new(transport: T, config: LinkConfig) -> Self; }
impl<T: Transport, C: Clock> Link<T, C> {
    pub fn with_clock(transport: T, config: LinkConfig, clock: C) -> Self;
    /// Envía y espera solo el ACK (0100, 0300, 0070, 0260|0| …).
    pub fn send_command(&mut self, payload: &str) -> Result<()>;
    /// Envía, espera ACK y una respuesta `expected` (código de función). `on_status` recibe los 0900.
    pub fn request(&mut self, payload: &str, expected: &'static str, on_status: &mut dyn FnMut(IntermediateStatus)) -> Result<String>;
    /// Envía, espera ACK y tramas `expected` hasta fin de lista (protocol-spec §4.2).
    pub fn request_list(&mut self, payload: &str, expected: &'static str) -> Result<Vec<String>>;
    pub fn config(&self) -> &LinkConfig;
    pub fn transport_mut(&mut self) -> &mut T;
    pub fn into_inner(self) -> T;
}
```

## 8. Clientes — `client/`

```rust
/// [raíz]
pub struct PosIntegrado<T: Transport, C: Clock = SystemClock> { … }
impl<T: Transport> PosIntegrado<T> {
    pub fn new(transport: T) -> Self;                         // LinkConfig::default()
    pub fn with_config(transport: T, config: LinkConfig) -> Self;
}
impl<T: Transport, C: Clock> PosIntegrado<T, C> {
    pub fn with_clock(transport: T, config: LinkConfig, clock: C) -> Self;
    pub fn poll(&mut self) -> Result<()>;
    pub fn load_keys(&mut self) -> Result<LoadKeysResponse>;
    pub fn sale(&mut self, req: &SaleRequest) -> Result<SaleResponse>;
    pub fn sale_with_status(&mut self, req: &SaleRequest, on_status: impl FnMut(IntermediateStatus)) -> Result<SaleResponse>;
    pub fn multicode_sale(&mut self, req: &MultiCodeSaleRequest) -> Result<MultiCodeSaleResponse>;
    pub fn multicode_sale_with_status(&mut self, req: &MultiCodeSaleRequest, on_status: impl FnMut(IntermediateStatus)) -> Result<MultiCodeSaleResponse>;
    pub fn last_sale(&mut self) -> Result<SaleResponse>;
    pub fn multicode_last_sale(&mut self, voucher: bool) -> Result<MultiCodeSaleResponse>;
    pub fn refund(&mut self, operation: OperationNumber) -> Result<RefundResponse>;
    pub fn totals(&mut self) -> Result<TotalsResponse>;
    pub fn sales_detail(&mut self, mode: SalesDetailMode) -> Result<Vec<DetailEntry>>;               // PrintOnPos ⇒ Ok(vec![])
    pub fn multicode_sales_detail(&mut self, mode: SalesDetailMode) -> Result<Vec<MultiCodeDetailEntry>>;
    pub fn close(&mut self) -> Result<CloseResponse>;
    pub fn set_normal_mode(&mut self) -> Result<()>;
    pub fn link_mut(&mut self) -> &mut Link<T, C>;
    pub fn into_inner(self) -> T;
}
#[cfg(feature = "serial")]
impl PosIntegrado<serial::SerialTransport> {
    pub fn open(path: &str) -> Result<Self>;                         // 115200
    /// Prueba `poll` en cada puerto de list_ports() (USB primero); devuelve el primero que responde.
    pub fn autoconnect(baud_rate: u32) -> Result<Option<(Self, serial::PortInfo)>>;
}

/// [raíz] (AUTO-01)
pub struct PosAutoservicio<T: Transport, C: Clock = SystemClock> { … }
// poll, load_keys, initialize, initialization_result, sale, sale_with_status, multicode_sale,
// last_sale(voucher: bool), refund_last(), close(voucher: bool)
```

`PosIntegrado` y `PosAutoservicio` son `Send` si `T` lo es (API-07).

## 9. Async (ASYNC-01, feature `tokio`)

Módulo `transbank_pos::tokio` con `AsyncTransport`, `AsyncPosIntegrado` con los mismos métodos `async`, reutilizando `protocol` y `message`. Se especifica en su tarea.

## 10. Simulador — `transbank-pos-sim`

Ver `docs/simulator-spec.md`. Superficie pública: `PosSimulator`, `TerminalKind`, `Scenario`, `SimulatedSale`, `SimTransport` (implementa `Transport`), `TcpServer`.
