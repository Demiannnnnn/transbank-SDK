# transbank-pos

> **Unofficial — not affiliated with, endorsed or supported by Transbank S.A.** "Transbank" is a trademark of its owner and is used only to describe compatibility. The protocol was derived from the official SDKs (BSD-3-Clause, see [NOTICE](NOTICE)). Always validate with your own terminal before charging real money.

Rust SDK for the **Transbank POS terminals** of Chile (POS Integrado: Verifone VX520/VX520c, Ingenico Desk 3500) connected by USB/serial port or through a TCP bridge:

- sans-IO protocol core: frame encoder/decoder (`STX payload ETX LRC`), typed requests and responses;
- link layer with ACK/NAK, retransmission of unacknowledged frames and configurable timeouts — an acknowledged sale is **never** resent;
- `PosIntegrado` client: poll, load keys, sale (with intermediate messages), multi-code sale, last sale, refund, totals, sales detail, close;
- serial (feature `serial`, default), TCP and in-memory transports; a simulator in the [`transbank-pos-sim`](https://crates.io/crates/transbank-pos-sim) crate.

**Status:** 1.0. Tested end to end against the bundled simulator (271 tests, property and mutation testing), but **never tested against a real Transbank terminal**. Reports from anyone with a POS are welcome.

## Installation

```toml
[dependencies]
transbank-pos = "1"
```

On Linux, listing USB ports needs `libudev` (`libudev-dev` / `systemd-devel`) and access to the port (`dialout` or `uucp` group).

## Example

```rust
use transbank_pos::{Error, PosIntegrado, SaleRequest};

let mut pos = PosIntegrado::open("/dev/ttyACM0")?; // COM3 on Windows
pos.poll()?;

let request = SaleRequest::new(12_000, "ABC123")?;
match pos.sale_with_status(&request, |status| eprintln!("POS: {}", status.message())) {
    Ok(sale) if sale.is_approved() => println!("approved: {:?}", sale.authorization_code),
    Ok(sale) => println!("declined: {}", sale.response_code),
    // The sale was acknowledged but the response was lost: the result is unknown.
    // Never resend blindly — check the last sale first.
    Err(Error::ResponseTimeout { .. }) => println!("{:?}", pos.last_sale()?),
    Err(err) => return Err(err),
}
```

Runnable examples (against `tbk-pos-sim` or a terminal): `cargo run --example sale` with `TBK_POS_TCP=127.0.0.1:7777` or `TBK_POS_PORT=/dev/ttyACM0`.

## Features

| Feature | Default | Enables |
|:--|:--|:--|
| `serial` | yes | `serial::SerialTransport`, `list_ports`, `PosIntegrado::open` / `autoconnect` |
| `serde` | no | `Serialize` for responses, `Deserialize` for requests |
| `tracing` | no | link events (`transbank_pos::link`), sensitive fields redacted |

MSRV: Rust 1.85. No `unsafe`, no panics on POS input, integer CLP amounts.

## Documentation

- API: <https://docs.rs/transbank-pos>
- Protocol specification and project: <https://github.com/Demiannnnnn/transbank-SDK>

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option. See [NOTICE](NOTICE) for third-party attributions.
