# Simulador de POS — `transbank-pos-sim`

Un POS falso que habla el protocolo de `docs/protocol-spec.md` desde el lado del terminal. Sirve para:

1. Los tests de integración del SDK (QA-02).
2. Los tests de las apps que usan el SDK (p. ej. `rework-sistema-facturacion`).
3. Probar la CLI y demos sin hardware.

No es un emulador de un modelo concreto: implementa la spec y permite **inyectar fallas**.

## 1. Núcleo sans-IO: `PosSimulator`

```rust
pub enum TerminalKind { Integrado, Autoservicio }

pub struct PosSimulator { /* estado */ }
impl PosSimulator {
    pub fn new(kind: TerminalKind) -> Self;            // escenario por defecto: todo aprobado
    pub fn with_scenario(self, s: Scenario) -> Self;
    pub fn set_scenario(&mut self, s: Scenario);
    /// Entrega bytes del host; devuelve los bytes que el POS "envía" de inmediato.
    pub fn feed(&mut self, input: &[u8]) -> Vec<u8>;
    pub fn sales(&self) -> &[SimulatedSale];          // ventas registradas desde el último cierre
    pub fn received_payloads(&self) -> &[String];     // para asserts
}
```

- Reutiliza `transbank_pos::protocol` (decoder, LRC, encoder) — una sola implementación (PROTO-02).
- **Responde ACK** a cada trama válida del host y **NAK** a las de LRC inválido.
- Tras enviar una respuesta, espera el ACK del host. Si en su lugar recibe NAK, **retransmite** la última respuesta (hasta 3 veces).
- Acepta las variantes de la venta de OPEN-01 (`0200|…|||0`, `0200|…||0|0`, con o sin `|` final, monto con o sin relleno).
- Estado: lista de ventas (para última venta, totales, detalle, anulación), contador de operación (desde `000001`), número de autorización (pseudoaleatorio determinista: derivado del contador, sin `rand`).

## 2. Escenarios

```rust
#[non_exhaustive]
pub enum Scenario {
    /// Todo se aprueba (por defecto).
    Approve,
    /// La próxima venta responde con este código (p. ej. 1 rechazada, 7 cancelada desde el POS).
    RejectNextSale(u16),
    /// Envía estos mensajes 0900 antes de la respuesta de venta (por defecto 78, 79, 81, 82).
    Intermediate(Vec<u16>),
    /// No envía ACK a los próximos N comandos.
    DropAcks(u8),
    /// Responde NAK a los próximos N comandos.
    NakNext(u8),
    /// Envía ACK pero nunca la respuesta (venta "colgada").
    NoResponse,
    /// La próxima respuesta sale con LRC corrupto N veces antes de salir bien.
    CorruptLrc(u8),
    /// Antepone bytes basura a la próxima respuesta.
    Garbage(Vec<u8>),
    /// Responde con otro código de función (para probar LINK-06).
    WrongFunctionCode(&'static str),
    /// Exige cierre: toda venta responde 74 hasta recibir 0500.
    RequireClose,
    /// Pide voucher: incluye un voucher de N líneas de 40 caracteres.
    WithVoucher(u8),
}
```

Los escenarios "próximos N" se consumen y luego vuelve a `Approve`.

## 3. Reglas de negocio simuladas (Integrado)

| Comando | Comportamiento |
|:--|:--|
| `0100`, `0300` | ACK |
| `0800` | `0810|00|597029414300|SIM00001` |
| `0200`/`0270` | monto < 50 ⇒ código 10; ticket > 6 ⇒ 70; si no, aprobada (`CR`/`VI` si el monto es par, `DB`/`DB` si es impar; cuotas solo crédito ≥ 10 000: 3 cuotas). Registra la venta |
| `0250` | última venta registrada; sin ventas ⇒ `0260|11|597029414300|SIM00001` |
| `1200` | operación existente de crédito no anulada ⇒ `1210|00|…`; débito ⇒ 8; anulada ⇒ 4; inexistente ⇒ 5 |
| `0700` | `0710|00|{count:03}|{total}||` (ventas no anuladas) |
| `0260|1|` | una `0261` por venta + dos tramas finales vacías; sin ventas ⇒ `0261|11|…` vacía |
| `0260|0|` | solo ACK |
| `0500` | `0510|00|597029414300|SIM00001|`; borra las ventas |

Autoservicio (AUTO-01): `0070` ACK, `0080` ⇒ `1080|90|{fecha}|{hora}`; venta con layout §7.1; `1200` anula la última.

Fechas: el simulador usa una fecha/hora fija configurable (`with_clock_fixed(PosDate, PosTime)`), por defecto `01012026`/`120000`, para que los tests sean deterministas.

## 4. `SimTransport`

`SimTransport::new(sim: PosSimulator, clock: ManualClock)` implementa `transbank_pos::Transport`: `write_all` alimenta `feed()` y encola la salida; `read_timeout` entrega lo encolado o, si no hay nada, avanza el reloj `timeout` y devuelve `Ok(0)`. `sim_mut()` permite cambiar el escenario entre operaciones.

## 5. Servidor TCP: `tbk-pos-sim`

```
tbk-pos-sim [--listen 127.0.0.1:7777] [--allow-remote] [--kind integrado|autoservicio]
            [--scenario approve|reject:<code>|intermediate|no-response|corrupt-lrc:<n>|require-close]
            [--delay-ms <n>]   # demora antes de cada respuesta (demos)
```

- Un cliente a la vez (un POS no atiende a dos cajas); un segundo cliente espera.
- Imprime en stdout cada payload recibido y enviado (`<-` / `->`), en español, con `--quiet` para silenciar.
- `--help` dice "Simulador NO oficial de un POS de Transbank" (LEG-03).
- Ctrl-C cierra limpio.
- Puente con la CLI y apps: `TcpTransport::connect("127.0.0.1:7777", …)`. Para probar software que solo habla serie, el README explica `socat` (`socat pty,link=/tmp/ttyV0,raw tcp:127.0.0.1:7777`) **solo en Linux**: en macOS `serialport` fija los baudios con el ioctl `IOSSIOSPEED`, que un pseudo-terminal rechaza ("Not a typewriter", verificado 2026-10-22 con socat 1.8.1.3); ahí se usa `--tcp`. No se implementa PTY en Rust (requeriría `unsafe`/`nix`).
