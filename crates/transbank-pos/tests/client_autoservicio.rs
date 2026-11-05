//! `PosAutoservicio` end to end against the simulator (QA-02): every public method,
//! declined and cancelled sales, timeouts and double charge prevention (LINK-03),
//! NAK, corrupt LRC, dropped ACKs, wrong function code, business rules and TCP.

use std::net::SocketAddr;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use transbank_pos::transport::{ManualClock, TcpTransport};
use transbank_pos::{
    CardBrand, CardType, Error, IntermediateStatus, LinkConfig, MultiCodeSaleRequest,
    PosAutoservicio, ResponseCode, SaleRequest,
};
use transbank_pos_sim::{PosSimulator, Scenario, SimTransport, TcpServer, TerminalKind};

type Pos = PosAutoservicio<SimTransport, ManualClock>;

fn pos_with(scenario: Scenario) -> (Pos, ManualClock) {
    let clock = ManualClock::new();
    let sim = PosSimulator::new(TerminalKind::Autoservicio).with_scenario(scenario);
    let pos = PosAutoservicio::with_clock(
        SimTransport::new(sim, clock.clone()),
        LinkConfig::default(),
        clock.clone(),
    );
    (pos, clock)
}

fn pos() -> Pos {
    pos_with(Scenario::Approve).0
}

fn set_scenario(pos: &mut Pos, scenario: Scenario) {
    pos.link_mut()
        .transport_mut()
        .sim_mut()
        .set_scenario(scenario);
}

fn assert_send<T: Send>() {}

#[test]
fn client_is_send() {
    assert_send::<PosAutoservicio<SimTransport, ManualClock>>();
}

#[test]
fn poll_and_initialize_and_result() {
    let mut pos = pos();
    pos.poll().unwrap();
    pos.initialize().unwrap();
    let res = pos.initialization_result().unwrap();
    assert!(res.is_approved());
    assert_eq!(res.response_code, ResponseCode::InitializationOk);
    assert!(res.date.is_some());
    assert!(res.time.is_some());

    let sim = pos.into_inner().into_inner();
    assert_eq!(sim.received_payloads(), ["0100", "0070", "0080"]);
}

#[test]
fn load_keys() {
    let mut pos = pos();
    let keys = pos.load_keys().unwrap();
    assert!(keys.is_approved());
    assert_eq!(keys.commerce_code.as_deref(), Some("597029414300"));
    assert_eq!(keys.terminal_id.as_deref(), Some("SIM00001"));
}

#[test]
fn approved_sale_and_installments() {
    let mut pos = pos();
    let req = SaleRequest::new(12_000, "ABC123").unwrap();
    let sale = pos.sale(&req).unwrap();
    assert!(sale.is_approved());
    assert_eq!(sale.function_code, "0210");
    assert_eq!(sale.ticket.as_deref(), Some("ABC123"));
    assert_eq!(sale.amount, Some(12_000));
    assert_eq!(sale.card_type, Some(CardType::Credit));
    assert_eq!(sale.card_brand, Some(CardBrand::Visa));
    assert_eq!(sale.installments_number, Some(3));
    assert_eq!(sale.installments_amount, Some(4_000));
    assert_eq!(sale.installments_type.as_deref(), Some("03"));
    assert_eq!(
        sale.installments_type_description.as_deref(),
        Some("CUOTAS SIN INTERES")
    );
    assert_eq!(sale.operation_number.as_deref(), Some("000001"));
    assert!(sale.authorization_code.is_some());

    let payloads = pos
        .link_mut()
        .transport_mut()
        .sim()
        .received_payloads()
        .to_vec();
    assert_eq!(payloads, ["0200|000012000|ABC123|0|0"]);
}

#[test]
fn sale_with_voucher_three_lines() {
    let mut pos = pos();
    let req = SaleRequest::new(10_000, "T001").unwrap().with_voucher(true);
    let sale = pos.sale(&req).unwrap();
    assert!(sale.is_approved());
    let lines = sale.voucher_lines().unwrap();
    assert_eq!(lines.len(), 3);
    for line in lines {
        assert_eq!(line.len(), 40);
        assert!(line.starts_with("SIMULADOR "));
    }
}

#[test]
fn declined_and_cancelled_sales_are_ok_not_approved() {
    let mut pos = pos_with(Scenario::RejectNextSale(1)).0;
    let sale = pos.sale(&SaleRequest::new(10_000, "T01").unwrap()).unwrap();
    assert!(!sale.is_approved());
    assert_eq!(sale.response_code, ResponseCode::Rejected);

    set_scenario(&mut pos, Scenario::RejectNextSale(7));
    let sale = pos.sale(&SaleRequest::new(10_000, "T02").unwrap()).unwrap();
    assert!(!sale.is_approved());
    assert_eq!(sale.response_code, ResponseCode::CanceledAtPos);
}

#[test]
fn sale_with_status_delivers_intermediate_messages() {
    let mut pos = pos_with(Scenario::Intermediate(vec![78, 81, 82])).0;
    let mut statuses = Vec::new();
    let req = SaleRequest::new(10_000, "T1").unwrap();
    let sale = pos
        .sale_with_status(&req, |status| statuses.push(status))
        .unwrap();
    assert!(sale.is_approved());
    assert_eq!(
        statuses,
        vec![
            IntermediateStatus::Code(ResponseCode::WaitingForCard),
            IntermediateStatus::Code(ResponseCode::RequestingPin),
            IntermediateStatus::Code(ResponseCode::SendingToHost),
        ]
    );
}

#[test]
fn multicode_sale() {
    let mut pos = pos();
    let req = MultiCodeSaleRequest::new(10_000, "T1", "597012345678").unwrap();
    let resp = pos.multicode_sale(&req).unwrap();
    assert!(resp.is_approved());
    assert_eq!(resp.sale.amount, Some(10_000));
    assert_eq!(resp.commerce_provider_code.as_deref(), Some("597012345678"));
}

#[test]
fn sale_timeout_leaves_result_unknown_and_last_sale_resolves() {
    let mut pos = pos_with(Scenario::NoResponse).0;
    let req = SaleRequest::new(12_000, "ABC123").unwrap();
    let err = pos.sale(&req).unwrap_err();
    assert!(matches!(err, Error::ResponseTimeout { .. }));

    // LINK-03: Pos did NOT resend the 0200 after ACK
    let sim = pos.link_mut().transport_mut().sim();
    let payloads = sim.received_payloads();
    assert_eq!(payloads, ["0200|000012000|ABC123|0|0"]);

    // Query last sale to resolve the charge
    set_scenario(&mut pos, Scenario::Approve);
    let last = pos.last_sale(false).unwrap();
    assert!(last.is_approved());
    assert_eq!(last.ticket.as_deref(), Some("ABC123"));
    assert_eq!(last.amount, Some(12_000));
}

#[test]
fn refund_timeout_leaves_result_unknown_and_link_03() {
    let mut pos = pos();
    // First make a sale so there is a sale to refund
    pos.sale(&SaleRequest::new(12_000, "ABC123").unwrap())
        .unwrap();

    set_scenario(&mut pos, Scenario::NoResponse);
    let err = pos.refund_last().unwrap_err();
    assert!(matches!(err, Error::ResponseTimeout { .. }));

    // LINK-03: exactly one 1200 received
    let sim = pos.link_mut().transport_mut().sim();
    let refunds_count = sim
        .received_payloads()
        .iter()
        .filter(|p| p.as_str() == "1200")
        .count();
    assert_eq!(refunds_count, 1);

    // Calling refund_last again now confirms it was already refunded (code 4)
    set_scenario(&mut pos, Scenario::Approve);
    let second = pos.refund_last().unwrap();
    assert!(!second.is_approved());
    assert_eq!(second.response_code, ResponseCode::AlreadyRefunded);
}

#[test]
fn refund_twice_gives_code_4() {
    let mut pos = pos();
    pos.sale(&SaleRequest::new(12_000, "T1").unwrap()).unwrap();

    let first = pos.refund_last().unwrap();
    assert!(first.is_approved());

    let second = pos.refund_last().unwrap();
    assert!(!second.is_approved());
    assert_eq!(second.response_code, ResponseCode::AlreadyRefunded);
}

#[test]
fn refund_debit_gives_code_8() {
    let mut pos = pos();
    // Odd amount is Debit in simulator
    pos.sale(&SaleRequest::new(12_001, "T1").unwrap()).unwrap();

    let refund = pos.refund_last().unwrap();
    assert!(!refund.is_approved());
    assert_eq!(refund.response_code, ResponseCode::DebitCannotBeRefunded);
}

#[test]
fn close_clears_sales_and_last_sale_yields_11() {
    let mut pos = pos();
    pos.sale(&SaleRequest::new(10_000, "T1").unwrap()).unwrap();
    let last = pos.last_sale(false).unwrap();
    assert!(last.is_approved());

    let close = pos.close(true).unwrap();
    assert!(close.is_approved());
    let close_lines = close.voucher_lines().unwrap();
    assert_eq!(close_lines.len(), 3);

    let last_after = pos.last_sale(false).unwrap();
    assert!(!last_after.is_approved());
    assert_eq!(last_after.response_code, ResponseCode::NoSale);
}

#[test]
fn corrupt_lrc_in_sale_recovers_with_nak_and_approves() {
    let mut pos = pos_with(Scenario::CorruptLrc(2)).0;
    let sale = pos.sale(&SaleRequest::new(10_000, "T1").unwrap()).unwrap();
    assert!(sale.is_approved());
}

#[test]
fn corrupt_lrc_in_refund_last_recovers_with_nak_and_approves() {
    let mut pos = pos();
    pos.sale(&SaleRequest::new(12_000, "ABC123").unwrap())
        .unwrap();
    set_scenario(&mut pos, Scenario::CorruptLrc(2));
    let refund = pos.refund_last().unwrap();
    assert!(refund.is_approved());
}

#[test]
fn corrupt_lrc_exhausted_in_sale_yields_bad_lrc_and_single_request() {
    let mut pos = pos_with(Scenario::CorruptLrc(4)).0;
    let err = pos
        .sale(&SaleRequest::new(10_000, "T1").unwrap())
        .unwrap_err();
    assert!(matches!(err, Error::BadLrc { count: 4 }));
    let sim = pos.into_inner().into_inner();
    let sales_received = sim
        .received_payloads()
        .iter()
        .filter(|p| p.starts_with("0200|"))
        .count();
    assert_eq!(sales_received, 1);
}

#[test]
fn nak_next_in_sale_recovers_and_registers_single_sale() {
    let mut pos = pos_with(Scenario::NakNext(2)).0;
    let sale = pos.sale(&SaleRequest::new(10_000, "T1").unwrap()).unwrap();
    assert!(sale.is_approved());
    let sim = pos.into_inner().into_inner();
    assert_eq!(sim.sales().len(), 1);
}

#[test]
fn drop_acks_in_sale_recovers_and_registers_single_sale() {
    let mut pos = pos_with(Scenario::DropAcks(2)).0;
    let sale = pos.sale(&SaleRequest::new(10_000, "T1").unwrap()).unwrap();
    assert!(sale.is_approved());
    let sim = pos.into_inner().into_inner();
    assert_eq!(sim.sales().len(), 1);
}

#[test]
fn wrong_function_code_is_unexpected_response() {
    let mut pos = pos_with(Scenario::WrongFunctionCode("0270")).0;
    let err = pos
        .sale(&SaleRequest::new(10_000, "T1").unwrap())
        .unwrap_err();
    assert!(matches!(err, Error::UnexpectedResponse { .. }));
}

type Server = JoinHandle<transbank_pos::Result<usize>>;

fn start_tcp_simulator() -> transbank_pos::Result<(SocketAddr, Server)> {
    let mut server = TcpServer::bind("127.0.0.1:0", false)?;
    let addr = server.local_addr()?;
    let handle = thread::spawn(move || {
        let mut sim = PosSimulator::new(TerminalKind::Autoservicio);
        server.serve_one(&mut sim)?;
        Ok(sim.received_payloads().len())
    });
    Ok((addr, handle))
}

#[test]
fn client_over_tcp_against_autoservicio_simulator() {
    let (addr, server) = start_tcp_simulator().unwrap();
    let transport = TcpTransport::connect(addr, Duration::from_secs(2)).unwrap();
    let mut pos = PosAutoservicio::new(transport);

    pos.poll().unwrap();
    pos.initialize().unwrap();
    let init_res = pos.initialization_result().unwrap();
    assert!(init_res.is_approved());

    let sale = pos
        .sale(&SaleRequest::new(10_000, "TCP01").unwrap())
        .unwrap();
    assert!(sale.is_approved());

    drop(pos);
    assert_eq!(server.join().unwrap().unwrap(), 4);
}
