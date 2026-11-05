# Changelog

All notable changes to this project are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Additive changes, compatible with 1.0.0. Tested against the bundled simulator only: **never tested against a real terminal**. Not released: no version bump, tag or publication yet (additive: next version would be a minor).

### Added

- `transbank-pos`: `PosAutoservicio` client (`poll`, `load_keys`, `initialize`, `initialization_result`, `sale`, `sale_with_status`, `multicode_sale`, `last_sale(voucher)`, `refund_last`, `close(voucher)`, and `open` with feature `serial`, 19200 baud); `SelfServiceSaleResponse`, `SelfServiceMultiCodeSaleResponse`, `InitializationResponse`, their payload builders and parsers; `FC_INITIALIZE`, `FC_INITIALIZATION_RESULT`, `FC_INITIALIZATION_RESPONSE`.
- `transbank-pos-cli`: `--autoservicio` (19200 baud by default), `init`, `init-result`, `refund-last`, and `--voucher` on `last-sale` and `close` (Autoservicio only).

### Changed

- `transbank-pos-sim`: `TerminalKind::Autoservicio` now simulates the terminal (`docs/protocol-spec.md` §7) instead of answering code 12 to everything but `0100`. Commands outside §7 still answer code 12.
- `transbank-pos-cli`: `--baud` has no fixed default; it depends on the terminal (115200 is unchanged for POS Integrado).

## [1.0.0] - 2026-10-22

First release. Tested against the bundled simulator only: **never tested against a real Transbank terminal** (see the open questions in `docs/protocol-spec.md` §10).

### Added

- `transbank-pos`:
  - sans-IO protocol core: LRC, frame encoder, partition-independent frame decoder (`FrameDecoder`), ISO-8859-1 decoding;
  - validated domain types (`Amount`, `Ticket`, `OperationNumber`, `CommerceCode`, `PosDate`, `PosTime`, `CardType`, `CardBrand`) and the 56 response codes (`ResponseCode`, `IntermediateStatus`);
  - payload builders and typed parsers for every POS Integrado command, with `Debug` that redacts account numbers and vouchers;
  - `Link`: ACK/NAK, retransmission of unacknowledged frames only, implicit ACK, bad-LRC NAK, response and sales-detail timeouts with an injectable clock;
  - `PosIntegrado` client with every POS Integrado operation;
  - transports: `TcpTransport`, `MemoryTransport` + `ManualClock`, and `serial::SerialTransport` with `list_ports`, `order_candidates` and `PosIntegrado::autoconnect` (feature `serial`);
  - features `serde` and `tracing` (redacted link traces).
- `transbank-pos-sim`: `PosSimulator` with failure scenarios, `SimTransport`, `TcpServer` and the `tbk-pos-sim` binary.
- `transbank-pos-cli`: the `tbk-pos` binary (serial, TCP or in-memory simulator; JSON output; redacted traces; confirmations; exit codes).
