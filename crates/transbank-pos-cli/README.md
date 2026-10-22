# transbank-pos-cli

> **Unofficial — not affiliated with, endorsed or supported by Transbank S.A.** "Transbank" is a trademark of its owner and is used only to describe compatibility.

`tbk-pos`: command-line tool (output in Spanish) to operate and diagnose Transbank POS Integrado terminals over a serial port, TCP or the built-in simulator. Built on [`transbank-pos`](https://crates.io/crates/transbank-pos).

## Installation

```bash
cargo install transbank-pos-cli
```

## Usage

```bash
tbk-pos ports                                    # serial ports (USB first)
tbk-pos autoconnect                              # find the POS
tbk-pos --port /dev/ttyACM0 poll                 # COM3 on Windows
tbk-pos --port /dev/ttyACM0 sale 1500 T00001 --status
tbk-pos --port /dev/ttyACM0 last-sale
tbk-pos --port /dev/ttyACM0 refund 42 --yes
tbk-pos --port /dev/ttyACM0 detail
tbk-pos --tcp 127.0.0.1:7777 totals              # serial-to-network bridge or tbk-pos-sim
tbk-pos --sim --scenario reject:1 sale 1000 T1   # in-memory simulator
```

Commands: `ports`, `autoconnect`, `poll`, `load-keys`, `sale`, `multicode-sale`, `last-sale`, `refund`, `totals`, `detail [--print]`, `close`, `normal-mode`, `raw <payload>`.

Options: `--json` (sensitive fields redacted), `--show-voucher`, `--trace-file <file>` (redacted link trace, filter with `RUST_LOG`), `--ack-timeout-ms`, `--response-timeout-s`, `--yes` (skip confirmation of refund/close/normal-mode).

Exit codes: `0` approved/ok · `1` not approved · `2` communication or protocol error · `64` usage error. If a sale times out, the result is unknown: run `last-sale` instead of repeating it.

Project: <https://github.com/Demiannnnnn/transbank-SDK>

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option. See [NOTICE](NOTICE).
