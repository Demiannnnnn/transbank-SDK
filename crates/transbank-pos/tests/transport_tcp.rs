//! `TcpTransport` end to end against the simulator TCP server, and its failure
//! modes. Only `127.0.0.1:0` is used (QA-03); every thread is joined.

use std::io::Read;
use std::net::{SocketAddr, TcpListener};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use transbank_pos::transport::TcpTransport;
use transbank_pos::{Error, LinkConfig, PosIntegrado, SaleRequest, SalesDetailMode, Transport};
use transbank_pos_sim::{PosSimulator, TcpServer, TerminalKind};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

fn fast_config() -> LinkConfig {
    let mut config = LinkConfig::default();
    config.ack_timeout = Duration::from_millis(50);
    config.response_timeout = Duration::from_secs(5);
    config.detail_tail = Duration::from_millis(50);
    config
}

/// Starts the simulator on `127.0.0.1:0` for one connection; returns the number of
/// payloads it received.
type Server = JoinHandle<transbank_pos::Result<usize>>;

fn simulator() -> transbank_pos::Result<(SocketAddr, Server)> {
    let mut server = TcpServer::bind("127.0.0.1:0", false)?;
    let addr = server.local_addr()?;
    let handle = thread::spawn(move || {
        let mut sim = PosSimulator::new(TerminalKind::Integrado);
        server.serve_one(&mut sim)?;
        Ok(sim.received_payloads().len())
    });
    Ok((addr, handle))
}

#[test]
fn client_over_tcp_against_the_simulator() {
    let (addr, server) = simulator().unwrap();
    let transport = TcpTransport::connect(addr, CONNECT_TIMEOUT).unwrap();
    let mut pos = PosIntegrado::with_config(transport, fast_config());

    pos.poll().unwrap();
    let sale = pos
        .sale(&SaleRequest::new(12_000, "ABC123").unwrap())
        .unwrap();
    assert!(sale.is_approved());
    assert_eq!(sale.terminal_id.as_deref(), Some("SIM00001"));
    let last = pos.last_sale().unwrap();
    assert_eq!(last.authorization_code, sale.authorization_code);
    let detail = pos.sales_detail(SalesDetailMode::SendToHost).unwrap();
    assert_eq!(detail.len(), 1);
    assert_eq!(detail[0].amount, Some(12_000));

    drop(pos);
    assert_eq!(server.join().unwrap().unwrap(), 4);
}

#[test]
fn closed_connection_is_unexpected_eof() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut frame = [0u8; 7];
        stream.read_exact(&mut frame).unwrap();
        frame
    });
    let transport = TcpTransport::connect(addr, CONNECT_TIMEOUT).unwrap();
    let mut pos = PosIntegrado::with_config(transport, fast_config());
    let err = pos.poll().unwrap_err();
    assert!(
        matches!(err, Error::Io(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof),
        "{err:?}"
    );
    assert_eq!(
        server.join().unwrap(),
        [0x02, b'0', b'1', b'0', b'0', 0x03, 0x02]
    );
}

#[test]
fn silent_server_is_no_ack_in_bounded_time() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut received = Vec::new();
        stream.read_to_end(&mut received).unwrap();
        received.len()
    });
    let transport = TcpTransport::connect(addr, CONNECT_TIMEOUT).unwrap();
    let mut pos = PosIntegrado::with_config(transport, fast_config());
    let started = Instant::now();
    assert!(matches!(
        pos.poll(),
        Err(Error::NoAck {
            attempts: 3,
            naks: 0
        })
    ));
    assert!(started.elapsed() < Duration::from_secs(2));
    drop(pos);
    assert_eq!(server.join().unwrap(), 21, "three transmissions of 0100");
}

#[test]
fn read_timeout_without_data_returns_zero() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut rest = Vec::new();
        stream.read_to_end(&mut rest).unwrap();
    });
    let mut transport = TcpTransport::connect(addr, CONNECT_TIMEOUT).unwrap();
    let mut buf = [0u8; 8];
    assert_eq!(transport.read_timeout(&mut buf, Duration::ZERO).unwrap(), 0);
    assert_eq!(
        transport
            .read_timeout(&mut buf, Duration::from_millis(20))
            .unwrap(),
        0
    );
    transport.discard_input().unwrap();
    drop(transport);
    server.join().unwrap();
}

#[test]
fn tcp_transport_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<TcpTransport>();
    assert_send::<PosIntegrado<TcpTransport>>();
}
