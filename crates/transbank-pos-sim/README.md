# transbank-pos-sim

> **Unofficial — not affiliated with, endorsed or supported by Transbank S.A.** "Transbank" is a trademark of its owner and is used only to describe compatibility.

Simulator of a Transbank **POS Integrado** for testing applications built on [`transbank-pos`](https://crates.io/crates/transbank-pos) without hardware:

- `PosSimulator`: sans-IO fake POS (ACK/NAK, retransmissions, sales, refunds, totals, detail, close) with injectable failures (`Scenario`: declined sale, intermediate messages, dropped ACKs, NAKs, no response, corrupted LRC, garbage, wrong function code, close required, voucher);
- `SimTransport`: the simulator as a `transbank_pos::Transport` with simulated time — timeouts take no real time;
- `TcpServer` and the `tbk-pos-sim` binary: the simulator over TCP (loopback only unless `--allow-remote`).

Deterministic: fixed date/time, authorization codes derived from the operation number, no randomness. POS Autoservicio is not simulated yet.

## Installation

```toml
[dev-dependencies]
transbank-pos-sim = "0.1"
```

```bash
cargo install transbank-pos-sim
tbk-pos-sim --listen 127.0.0.1:7777 --scenario intermediate
```

## Example

```rust
use transbank_pos::transport::ManualClock;
use transbank_pos::{LinkConfig, PosIntegrado, SaleRequest};
use transbank_pos_sim::{PosSimulator, Scenario, SimTransport, TerminalKind};

let clock = ManualClock::new();
let sim = PosSimulator::new(TerminalKind::Integrado).with_scenario(Scenario::RejectNextSale(1));
let mut pos = PosIntegrado::with_clock(SimTransport::new(sim, clock.clone()), LinkConfig::default(), clock);

let sale = pos.sale(&SaleRequest::new(12_000, "ABC123")?)?;
assert!(!sale.is_approved()); // declined: a response, not an error
```

To test software that only speaks serial, bridge a pseudo-terminal to the TCP server: `socat pty,link=/tmp/ttyV0,raw tcp:127.0.0.1:7777` (Linux/macOS).

Project and specification: <https://github.com/Demiannnnnn/transbank-SDK>

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option. See [NOTICE](NOTICE).
