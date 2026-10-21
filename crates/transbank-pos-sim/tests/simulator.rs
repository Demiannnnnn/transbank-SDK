//! Simulator behaviour: POS-side link, business rules (`docs/simulator-spec.md` §3),
//! scenarios (§2), `SimTransport` with the real link, and the TCP server.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::Command;

use transbank_pos::message::{parse_detail_entry, parse_sale};
use transbank_pos::protocol::{ACK, FrameDecoder, LinkEvent, NAK, decode_latin1, encode_frame};
use transbank_pos::transport::ManualClock;
use transbank_pos::{CardBrand, CardType, Link, LinkConfig, ResponseCode};
use transbank_pos_sim::{PosSimulator, Scenario, SimTransport, TcpServer, TerminalKind};

/// Encoded frame; an invalid payload yields an empty frame (and a failing test).
fn frame(payload: &str) -> Vec<u8> {
    encode_frame(payload).unwrap_or_default()
}

fn events(bytes: &[u8]) -> Vec<LinkEvent> {
    let mut decoder = FrameDecoder::new();
    let mut events = Vec::new();
    decoder.push(bytes, &mut events);
    events
}

fn frames_in(bytes: &[u8]) -> Vec<String> {
    events(bytes)
        .into_iter()
        .filter_map(|event| match event {
            LinkEvent::Frame(payload) => Some(decode_latin1(&payload)),
            _ => None,
        })
        .collect()
}

/// Sends a command like a well-behaved host (ACK to every response frame) and
/// returns whether the POS acknowledged it and the response payloads.
fn exchange(sim: &mut PosSimulator, payload: &str) -> (bool, Vec<String>) {
    let mut output = sim.feed(&frame(payload));
    let acked = output.first() == Some(&ACK);
    let mut responses = Vec::new();
    loop {
        let new = frames_in(&output);
        if new.is_empty() {
            return (acked, responses);
        }
        responses.extend(new);
        output = sim.feed(&[ACK]);
    }
}

fn sim() -> PosSimulator {
    PosSimulator::new(TerminalKind::Integrado)
}

fn field(payload: &str, index: usize) -> String {
    payload.split('|').nth(index).unwrap_or_default().to_owned()
}

#[test]
fn sale_frame_gets_ack_and_valid_response() {
    let mut sim = sim();
    let output = sim.feed(&frame("0200|000012000|ABC123||0|0"));
    assert_eq!(output.first(), Some(&ACK));
    let frames = frames_in(&output);
    assert_eq!(frames.len(), 1, "response with a valid LRC");
    let sale = parse_sale(&frames[0]).unwrap();
    assert!(sale.is_approved());
    assert_eq!(sale.amount, Some(12_000));
    assert_eq!(sale.terminal_id.as_deref(), Some("SIM00001"));
}

#[test]
fn response_is_retransmitted_on_nak_up_to_three_times() {
    let mut sim = sim();
    let first = sim.feed(&frame("0800"));
    let response = first.get(1..).unwrap().to_vec();
    for _ in 0..3 {
        assert_eq!(sim.feed(&[NAK]), response);
    }
    assert_eq!(sim.feed(&[NAK]), Vec::<u8>::new(), "gives up after 3");
    assert_eq!(sim.feed(&[ACK]), Vec::<u8>::new());
}

#[test]
fn without_host_ack_the_next_frame_is_not_sent() {
    let mut sim = sim().with_scenario(Scenario::Intermediate(vec![78, 81]));
    let output = sim.feed(&frame("0200|000012000|ABC123||0|1"));
    assert_eq!(frames_in(&output), ["0900|78"]);
    assert_eq!(frames_in(&sim.feed(&[ACK])), ["0900|81"]);
    let last = frames_in(&sim.feed(&[ACK]));
    assert_eq!(field(&last[0], 0), "0210");
}

#[test]
fn bad_lrc_from_host_gets_nak() {
    let mut sim = sim();
    let mut bad = frame("0100");
    *bad.last_mut().unwrap() ^= 0xFF;
    assert_eq!(sim.feed(&bad), [NAK]);
    assert_eq!(sim.received_payloads(), Vec::<String>::new());
}

#[test]
fn poll_normal_mode_and_print_detail_only_ack() {
    let mut sim = sim();
    for payload in ["0100", "0300", "0260|0|", "0290|0|"] {
        assert_eq!(exchange(&mut sim, payload), (true, vec![]), "{payload}");
    }
}

#[test]
fn load_keys() {
    let (_, responses) = exchange(&mut sim(), "0800");
    assert_eq!(responses, ["0810|00|597029414300|SIM00001"]);
}

#[test]
fn sale_validation_rules() {
    let mut sim = sim();
    let (_, low) = exchange(&mut sim, "0200|000000049|1||0|0");
    assert_eq!(field(&low[0], 1), "10");
    let (_, long) = exchange(&mut sim, "0200|000001000|1234567||0|0");
    assert_eq!(field(&long[0], 1), "70");
    assert_eq!(sim.sales(), []);
}

#[test]
fn even_amount_is_credit_with_installments_odd_is_debit() {
    let mut sim = sim();
    let (_, credit) = exchange(&mut sim, "0200|000012000|1||0|0");
    let credit = parse_sale(&credit[0]).unwrap();
    assert_eq!(credit.card_type, Some(CardType::Credit));
    assert_eq!(credit.card_brand, Some(CardBrand::Visa));
    assert_eq!(credit.installments_number, Some(3));
    assert_eq!(credit.installments_amount, Some(4_000));
    assert_eq!(credit.operation_number.as_deref(), Some("000001"));

    let (_, small) = exchange(&mut sim, "0200|000000500|2||0|0");
    assert_eq!(parse_sale(&small[0]).unwrap().installments_number, Some(0));

    let (_, debit) = exchange(&mut sim, "0200|000001001|3||0|0");
    let debit = parse_sale(&debit[0]).unwrap();
    assert_eq!(debit.card_type, Some(CardType::Debit));
    assert_eq!(debit.card_brand, Some(CardBrand::Redcompra));
    assert_eq!(debit.operation_number.as_deref(), Some("000003"));
    assert_eq!(sim.sales().len(), 3);
}

#[test]
fn authorization_is_deterministic() {
    let (_, a) = exchange(&mut sim(), "0200|000012000|1||0|0");
    let (_, b) = exchange(&mut sim(), "0200|000012000|1||0|0");
    assert_eq!(a, b);
}

#[test]
fn last_sale() {
    let mut sim = sim();
    assert_eq!(
        exchange(&mut sim, "0250|").1,
        ["0260|11|597029414300|SIM00001"]
    );
    exchange(&mut sim, "0200|000012000|ABC123||0|0");
    let (_, last) = exchange(&mut sim, "0250|");
    let last = parse_sale(&last[0]).unwrap();
    assert_eq!(last.function_code, "0260");
    assert_eq!(last.ticket.as_deref(), Some("ABC123"));
    assert_eq!(last.amount, Some(12_000));
}

#[test]
fn refunds() {
    let mut sim = sim();
    exchange(&mut sim, "0200|000012000|1||0|0"); // op 1, credit
    exchange(&mut sim, "0200|000001001|2||0|0"); // op 2, debit
    let code =
        |sim: &mut PosSimulator, op: &str| field(&exchange(sim, &format!("1200|{op}|")).1[0], 1);
    assert_eq!(code(&mut sim, "000001"), "00");
    assert_eq!(code(&mut sim, "000001"), "04");
    assert_eq!(code(&mut sim, "000002"), "08");
    assert_eq!(code(&mut sim, "000099"), "05");
    assert!(sim.sales()[0].refunded);
}

#[test]
fn totals_exclude_refunded_sales() {
    let mut sim = sim();
    assert_eq!(exchange(&mut sim, "0700||").1, ["0710|00|000|0||"]);
    exchange(&mut sim, "0200|000012000|1||0|0");
    exchange(&mut sim, "0200|000003000|2||0|0");
    exchange(&mut sim, "1200|000001|");
    assert_eq!(exchange(&mut sim, "0700||").1, ["0710|00|001|3000||"]);
}

#[test]
fn sales_detail() {
    let mut sim = sim();
    assert_eq!(
        exchange(&mut sim, "0260|1|").1,
        ["0261|11||||||||||||||||||"]
    );
    for ticket in ["1", "2", "3"] {
        exchange(&mut sim, &format!("0200|000012000|{ticket}||0|0"));
    }
    let (_, detail) = exchange(&mut sim, "0260|1|");
    assert_eq!(detail.len(), 5);
    let entries: Vec<_> = detail
        .iter()
        .map(|p| parse_detail_entry(p).unwrap())
        .collect();
    assert_eq!(entries[0].ticket.as_deref(), Some("1"));
    assert_eq!(entries[2].operation_number.as_deref(), Some("000003"));
    assert_eq!(entries[0].installments_number, Some(3));
    assert_eq!(entries[0].installments_amount, Some(4_000));
    assert!(entries[3].authorization_code.is_none());
    assert!(entries[4].authorization_code.is_none());
}

#[test]
fn close_clears_sales() {
    let mut sim = sim();
    exchange(&mut sim, "0200|000012000|1||0|0");
    assert_eq!(
        exchange(&mut sim, "0500||").1,
        ["0510|00|597029414300|SIM00001|"]
    );
    assert_eq!(sim.sales(), []);
    assert_eq!(
        exchange(&mut sim, "0250|").1,
        ["0260|11|597029414300|SIM00001"]
    );
}

#[test]
fn multicode_sale_and_detail() {
    let mut sim = sim();
    let (_, sale) = exchange(&mut sim, "0270|000012000|7||0|0|597012345678");
    assert_eq!(field(&sale[0], 0), "0271");
    assert_eq!(field(&sale[0], 21), "597012345678");
    let (_, detail) = exchange(&mut sim, "0290|1|");
    assert_eq!(detail.len(), 3);
    assert_eq!(field(&detail[0], 0), "0291");
    assert_eq!(field(&detail[0], 20), "597012345678");
    let (_, last) = exchange(&mut sim, "0280|0");
    assert_eq!(field(&last[0], 0), "0281");
}

#[test]
fn accepts_the_three_sale_variants() {
    for payload in [
        "0200|000012000|ABC123|||0",
        "0200|12000|ABC123||0|1|",
        "0200|000012000|ABC123||0|0",
    ] {
        let mut sim = sim();
        let (acked, responses) = exchange(&mut sim, payload);
        assert!(acked);
        let sale = parse_sale(responses.last().unwrap()).unwrap();
        assert!(sale.is_approved(), "{payload}");
        assert_eq!(sale.amount, Some(12_000));
    }
}

#[test]
fn scenario_reject_next_sale() {
    let mut sim = sim().with_scenario(Scenario::RejectNextSale(7));
    let (_, first) = exchange(&mut sim, "0200|000012000|1||0|0");
    assert_eq!(
        parse_sale(&first[0]).unwrap().response_code,
        ResponseCode::CanceledAtPos
    );
    assert_eq!(sim.scenario(), &Scenario::Approve);
    let (_, second) = exchange(&mut sim, "0200|000012000|1||0|0");
    assert!(parse_sale(&second[0]).unwrap().is_approved());
}

#[test]
fn scenario_intermediate_and_default_codes() {
    let (_, default) = exchange(&mut sim(), "0200|000012000|1||0|1");
    assert_eq!(&default[..4], ["0900|78", "0900|79", "0900|81", "0900|82"]);
    let (_, none) = exchange(&mut sim(), "0200|000012000|1||0|0");
    assert_eq!(none.len(), 1);
    let mut custom = sim().with_scenario(Scenario::Intermediate(vec![84]));
    let (_, custom) = exchange(&mut custom, "0200|000012000|1||0|1");
    assert_eq!(&custom[..1], ["0900|84"]);
}

#[test]
fn scenario_drop_acks() {
    let mut sim = sim().with_scenario(Scenario::DropAcks(2));
    assert_eq!(sim.feed(&frame("0100")), Vec::<u8>::new());
    assert_eq!(sim.feed(&frame("0100")), Vec::<u8>::new());
    assert_eq!(sim.feed(&frame("0100")), [ACK]);
}

#[test]
fn scenario_nak_next() {
    let mut sim = sim().with_scenario(Scenario::NakNext(1));
    assert_eq!(sim.feed(&frame("0100")), [NAK]);
    assert_eq!(sim.feed(&frame("0100")), [ACK]);
}

#[test]
fn scenario_no_response() {
    let mut sim = sim().with_scenario(Scenario::NoResponse);
    assert_eq!(sim.feed(&frame("0200|000012000|1||0|0")), [ACK]);
    assert_eq!(sim.feed(&frame("0800")), [ACK]);
}

#[test]
fn scenario_corrupt_lrc() {
    let mut sim = sim().with_scenario(Scenario::CorruptLrc(2));
    let output = sim.feed(&frame("0800"));
    assert!(matches!(
        events(&output)[..],
        [LinkEvent::Ack, LinkEvent::BadLrc { .. }]
    ));
    assert!(matches!(
        events(&sim.feed(&[NAK]))[..],
        [LinkEvent::BadLrc { .. }]
    ));
    assert_eq!(
        frames_in(&sim.feed(&[NAK])),
        ["0810|00|597029414300|SIM00001"]
    );
}

#[test]
fn scenario_garbage() {
    let mut sim = sim().with_scenario(Scenario::Garbage(b"zz".to_vec()));
    let output = sim.feed(&frame("0800"));
    let mut expected = vec![ACK, b'z', b'z'];
    expected.extend(frame("0810|00|597029414300|SIM00001"));
    assert_eq!(output, expected);
    assert_eq!(sim.scenario(), &Scenario::Approve);
}

#[test]
fn scenario_wrong_function_code() {
    let mut sim = sim().with_scenario(Scenario::WrongFunctionCode("0810"));
    let (_, responses) = exchange(&mut sim, "0200|000012000|1||0|0");
    assert_eq!(field(&responses[0], 0), "0810");
    let (_, next) = exchange(&mut sim, "0200|000012000|1||0|0");
    assert_eq!(field(&next[0], 0), "0210");
}

#[test]
fn scenario_require_close() {
    let mut sim = sim().with_scenario(Scenario::RequireClose);
    let (_, sale) = exchange(&mut sim, "0200|000012000|1||0|0");
    let sale = parse_sale(&sale[0]).unwrap();
    assert_eq!(sale.response_code, ResponseCode::MustRunClose);
    assert!(sale.response_code.requires_close());
    exchange(&mut sim, "0500||");
    let (_, sale) = exchange(&mut sim, "0200|000012000|1||0|0");
    assert!(parse_sale(&sale[0]).unwrap().is_approved());
}

#[test]
fn scenario_with_voucher() {
    let mut sim = sim().with_scenario(Scenario::WithVoucher(2));
    let (_, sale) = exchange(&mut sim, "0200|000012000|ABC123||0|0");
    let sale = parse_sale(&sale[0]).unwrap();
    let lines = sale.voucher_lines().unwrap();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("SIMULADOR 01 TICKET ABC123"));
    let (_, asked) = exchange(
        &mut PosSimulator::new(TerminalKind::Integrado),
        "0200|000012000|1||1|0",
    );
    assert_eq!(
        parse_sale(&asked[0])
            .unwrap()
            .voucher_lines()
            .map(|l| l.len()),
        Some(3)
    );
}

#[test]
fn autoservicio_is_pending() {
    let mut sim = PosSimulator::new(TerminalKind::Autoservicio);
    assert_eq!(exchange(&mut sim, "0100"), (true, vec![]));
    assert_eq!(exchange(&mut sim, "0200|000012000|1||0|0").1, ["0210|12"]);
}

#[test]
fn sim_transport_with_the_real_link() {
    let clock = ManualClock::new();
    let transport = SimTransport::new(sim(), clock.clone());
    let mut link = Link::with_clock(transport, LinkConfig::default(), clock.clone());
    let mut statuses = Vec::new();
    let response = link
        .request("0200|000012000|ABC123||0|1", "0210", &mut |s| {
            statuses.push(s);
        })
        .unwrap();
    assert_eq!(statuses.len(), 4);
    let sale = parse_sale(&response).unwrap();
    assert!(sale.is_approved());
    assert_eq!(link.transport_mut().sim().sales().len(), 1);

    link.transport_mut()
        .sim_mut()
        .set_scenario(Scenario::NoResponse);
    assert!(matches!(
        link.request("0200|000012000|ABC123||0|0", "0210", &mut |_| {}),
        Err(transbank_pos::Error::ResponseTimeout { .. })
    ));
    let sale_frames = link
        .transport_mut()
        .sim()
        .received_payloads()
        .iter()
        .filter(|p| p.starts_with("0200"))
        .count();
    assert_eq!(sale_frames, 2, "the hung sale was sent once");
}

#[test]
fn tcp_server_answers_poll() {
    let mut server = TcpServer::bind("127.0.0.1:0", false).unwrap();
    let addr = server.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let mut sim = PosSimulator::new(TerminalKind::Integrado);
        server
            .serve_one(&mut sim)
            .map(|()| sim.received_payloads().to_vec())
    });
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(&frame("0100")).unwrap();
    let mut ack = [0u8; 1];
    stream.read_exact(&mut ack).unwrap();
    assert_eq!(ack, [ACK]);
    drop(stream);
    assert_eq!(handle.join().unwrap().unwrap(), ["0100"]);
}

#[test]
fn tcp_server_rejects_remote_without_flag() {
    assert!(matches!(
        TcpServer::bind("0.0.0.0:0", false),
        Err(transbank_pos::Error::InvalidArgument {
            field: "listen",
            ..
        })
    ));
}

#[test]
fn binary_help_declares_unofficial() {
    let output = Command::new(env!("CARGO_BIN_EXE_tbk-pos-sim"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("NO oficial"), "{help}");
}

#[test]
fn new_command_supersedes_a_pending_response() {
    let mut sim = sim().with_scenario(Scenario::Intermediate(vec![78, 81]));
    let first = sim.feed(&frame("0200|000012000|1||0|1"));
    assert_eq!(frames_in(&first), ["0900|78"]);
    // The host gives up and polls instead of acknowledging the 0900.
    assert_eq!(sim.feed(&frame("0100")), [ACK]);
    assert_eq!(
        sim.feed(&[ACK]),
        Vec::<u8>::new(),
        "nothing pending any more"
    );
}

#[test]
fn malformed_request_answers_code_61() {
    let (_, responses) = exchange(&mut sim(), "0200|12a|1||0|0");
    assert_eq!(responses, ["0210|61"]);
}

#[test]
fn unknown_command_is_only_acknowledged() {
    assert_eq!(exchange(&mut sim(), "9999|x"), (true, vec![]));
}

#[test]
fn amount_above_nine_digits_answers_68() {
    let (_, responses) = exchange(&mut sim(), "0200|1000000000|1||0|0");
    assert_eq!(field(&responses[0], 1), "68");
}

#[test]
fn fixed_clock_is_reported() {
    let date = transbank_pos::PosDate::parse("31122030").unwrap();
    let time = transbank_pos::PosTime::parse("235959").unwrap();
    let mut sim = sim().with_clock_fixed(date, time);
    let (_, sale) = exchange(&mut sim, "0200|000012000|1||0|0");
    let sale = parse_sale(&sale[0]).unwrap();
    assert_eq!(sale.real_date, Some(date));
    assert_eq!(sale.accounting_date, Some(date));
    assert_eq!(sale.real_time, Some(time));
}

#[test]
fn operation_numbers_are_consecutive_and_six_digits() {
    let mut sim = sim();
    for expected in 1..=3 {
        let (_, sale) = exchange(&mut sim, "0200|000012000|1||0|0");
        assert_eq!(field(&sale[0], 10), format!("{expected:06}"));
    }
}

#[test]
fn simulator_state_survives_reconnections() {
    let mut server = TcpServer::bind("127.0.0.1:0", false).unwrap();
    let addr = server.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let mut sim = PosSimulator::new(TerminalKind::Integrado);
        server.serve_one(&mut sim)?;
        server.serve_one(&mut sim)?;
        Ok::<_, transbank_pos::Error>(sim.sales().len())
    });
    for _ in 0..2 {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(&frame("0200|000001000|1||0|0")).unwrap();
        let mut buf = [0u8; 256];
        let mut received = Vec::new();
        while frames_in(&received).is_empty() {
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0, "connection closed early");
            received.extend_from_slice(&buf[..n]);
        }
        stream.write_all(&[ACK]).unwrap();
    }
    assert_eq!(handle.join().unwrap().unwrap(), 2);
}

#[test]
fn tcp_observer_sees_input_and_output() {
    let mut server = TcpServer::bind("127.0.0.1:0", false).unwrap();
    let addr = server.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let mut sim = PosSimulator::new(TerminalKind::Integrado);
        let mut seen = Vec::new();
        server.serve_one_with(&mut sim, &mut |input, output| {
            seen.push((input.to_vec(), output.to_vec()));
        })?;
        Ok::<_, transbank_pos::Error>(seen)
    });
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(&frame("0100")).unwrap();
    let mut ack = [0u8; 1];
    stream.read_exact(&mut ack).unwrap();
    drop(stream);
    let seen = handle.join().unwrap().unwrap();
    assert_eq!(seen, [(frame("0100"), vec![ACK])]);
}
