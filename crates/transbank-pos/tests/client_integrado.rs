//! `PosIntegrado` end to end against the simulator (QA-02): every public method,
//! declined sales, timeouts and recovery, NAK, bad LRC and wrong responses.

use std::time::Duration;

use transbank_pos::transport::ManualClock;
use transbank_pos::{
    CardType, Error, IntermediateStatus, LinkConfig, MultiCodeSaleRequest, OperationNumber,
    PosIntegrado, ResponseCode, SaleRequest, SalesDetailMode,
};
use transbank_pos_sim::{PosSimulator, Scenario, SimTransport, TerminalKind};

type Pos = PosIntegrado<SimTransport, ManualClock>;

fn pos_with(scenario: Scenario) -> (Pos, ManualClock) {
    let clock = ManualClock::new();
    let sim = PosSimulator::new(TerminalKind::Integrado).with_scenario(scenario);
    let pos = PosIntegrado::with_clock(
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

fn sale_request(amount: u32, ticket: &str) -> SaleRequest {
    SaleRequest::new(amount, ticket).unwrap_or_else(|_| unreachable!("valid request"))
}

#[test]
fn poll_and_normal_mode() {
    let mut pos = pos();
    pos.poll().unwrap();
    pos.set_normal_mode().unwrap();
    let sim = pos.into_inner().into_inner();
    assert_eq!(sim.received_payloads(), ["0100", "0300"]);
}

#[test]
fn load_keys() {
    let keys = pos().load_keys().unwrap();
    assert!(keys.is_approved());
    assert_eq!(keys.commerce_code.as_deref(), Some("597029414300"));
    assert_eq!(keys.terminal_id.as_deref(), Some("SIM00001"));
}

#[test]
fn approved_sale() {
    let mut pos = pos();
    let sale = pos.sale(&sale_request(12_000, "ABC123")).unwrap();
    assert!(sale.is_approved());
    assert_eq!(sale.function_code, "0210");
    assert_eq!(sale.ticket.as_deref(), Some("ABC123"));
    assert_eq!(sale.amount, Some(12_000));
    assert_eq!(sale.card_type, Some(CardType::Credit));
    assert_eq!(sale.installments_number, Some(3));
    assert_eq!(sale.operation_number.as_deref(), Some("000001"));
    assert_eq!(sale.authorization_code.as_ref().map(String::len), Some(6));
    assert_eq!(
        sale.real_date.map(|d| d.to_string()).as_deref(),
        Some("01-01-2026")
    );
    let sent = pos
        .link_mut()
        .transport_mut()
        .sim()
        .received_payloads()
        .to_vec();
    assert_eq!(sent, ["0200|000012000|ABC123||0|0"]);
}

#[test]
fn declined_sale_is_ok() {
    let mut pos = pos_with(Scenario::RejectNextSale(1)).0;
    let sale = pos.sale(&sale_request(12_000, "1")).unwrap();
    assert!(!sale.is_approved());
    assert_eq!(sale.response_code, ResponseCode::Rejected);
}

#[test]
fn sale_with_status_reports_intermediate_messages() {
    let mut pos = pos_with(Scenario::Intermediate(vec![78, 81, 82])).0;
    let mut statuses = Vec::new();
    let sale = pos
        .sale_with_status(&sale_request(12_000, "1"), |status| statuses.push(status))
        .unwrap();
    assert!(sale.is_approved());
    assert_eq!(
        statuses,
        [78, 81, 82].map(|code| IntermediateStatus::Code(ResponseCode::from_code(code)))
    );
    let sent = pos
        .link_mut()
        .transport_mut()
        .sim()
        .received_payloads()
        .to_vec();
    assert_eq!(sent, ["0200|000012000|000001||0|1"]);
}

#[test]
fn hung_sale_times_out_and_last_sale_resolves_it() {
    let (mut pos, clock) = pos_with(Scenario::NoResponse);
    let request = sale_request(12_000, "ABC123");
    assert!(matches!(
        pos.sale(&request),
        Err(Error::ResponseTimeout { .. })
    ));
    assert_eq!(clock.now(), Duration::from_secs(150));

    set_scenario(&mut pos, Scenario::Approve);
    let last = pos.last_sale().unwrap();
    assert_eq!(last.function_code, "0260");
    assert!(last.is_approved());
    assert_eq!(last.ticket.as_deref(), Some("ABC123"));
    assert_eq!(last.amount, Some(12_000));
    let sales_sent = pos
        .link_mut()
        .transport_mut()
        .sim()
        .received_payloads()
        .iter()
        .filter(|payload| payload.starts_with("0200"))
        .count();
    assert_eq!(sales_sent, 1, "the sale is never resent (LINK-03)");
}

#[test]
fn last_sale_without_sales() {
    let last = pos().last_sale().unwrap();
    assert_eq!(last.response_code, ResponseCode::NoSale);
    assert!(!last.is_approved());
}

#[test]
fn naks_are_retried() {
    let mut pos = pos_with(Scenario::NakNext(2)).0;
    assert!(pos.sale(&sale_request(12_000, "1")).unwrap().is_approved());
}

#[test]
fn missing_acks_fail_with_no_ack() {
    let (mut pos, clock) = pos_with(Scenario::DropAcks(3));
    assert!(matches!(
        pos.sale(&sale_request(12_000, "1")),
        Err(Error::NoAck {
            attempts: 3,
            naks: 0
        })
    ));
    assert_eq!(clock.now(), Duration::from_secs(6));
}

#[test]
fn corrupted_lrc_is_recovered() {
    let mut pos = pos_with(Scenario::CorruptLrc(1)).0;
    assert!(pos.sale(&sale_request(12_000, "1")).unwrap().is_approved());
}

#[test]
fn wrong_function_code_is_unexpected_response() {
    let mut pos = pos_with(Scenario::WrongFunctionCode("0810")).0;
    assert!(matches!(
        pos.sale(&sale_request(12_000, "1")),
        Err(Error::UnexpectedResponse {
            expected: "0210",
            ..
        })
    ));
}

#[test]
fn require_close_then_close() {
    let mut pos = pos_with(Scenario::RequireClose).0;
    let sale = pos.sale(&sale_request(12_000, "1")).unwrap();
    assert!(!sale.is_approved());
    assert!(sale.response_code.requires_close());
    let close = pos.close().unwrap();
    assert!(close.is_approved());
    assert_eq!(close.terminal_id.as_deref(), Some("SIM00001"));
    assert!(pos.sale(&sale_request(12_000, "1")).unwrap().is_approved());
}

#[test]
fn refunds() {
    let mut pos = pos();
    let credit = pos.sale(&sale_request(12_000, "1")).unwrap();
    let debit = pos.sale(&sale_request(3_001, "2")).unwrap();
    let op = |sale: &transbank_pos::SaleResponse| -> OperationNumber {
        sale.operation_number
            .as_deref()
            .unwrap_or_default()
            .parse()
            .unwrap()
    };
    let refund = pos.refund(op(&credit)).unwrap();
    assert!(refund.is_approved());
    assert_eq!(refund.operation_number.as_deref(), Some("000003"));
    assert_eq!(refund.authorization_code.as_ref().map(String::len), Some(6));
    let refund = pos.refund(op(&debit)).unwrap();
    assert_eq!(refund.response_code, ResponseCode::DebitCannotBeRefunded);
}

#[test]
fn sales_detail_and_totals() {
    let mut pos = pos();
    assert_eq!(pos.sales_detail(SalesDetailMode::SendToHost).unwrap(), []);
    for (amount, ticket) in [(12_000, "1"), (3_001, "2"), (500, "3")] {
        pos.sale(&sale_request(amount, ticket)).unwrap();
    }
    let detail = pos.sales_detail(SalesDetailMode::SendToHost).unwrap();
    let tickets: Vec<_> = detail
        .iter()
        .map(|e| e.ticket.clone().unwrap_or_default())
        .collect();
    assert_eq!(tickets, ["000001", "000002", "000003"]);
    assert!(detail.iter().all(transbank_pos::DetailEntry::is_approved));
    assert_eq!(detail[1].card_type, Some(CardType::Debit));

    assert_eq!(pos.sales_detail(SalesDetailMode::PrintOnPos).unwrap(), []);

    let totals = pos.totals().unwrap();
    assert_eq!(totals.tx_count, Some(3));
    assert_eq!(totals.tx_total, Some(15_501));
}

#[test]
fn multicode_operations() {
    let mut pos = pos();
    let request = MultiCodeSaleRequest::new(12_000, "7", "597012345678").unwrap();
    let sale = pos.multicode_sale(&request).unwrap();
    assert!(sale.is_approved());
    assert_eq!(sale.sale.function_code, "0271");
    assert_eq!(sale.commerce_provider_code.as_deref(), Some("597012345678"));
    assert_eq!(sale.change, Some(0));

    let mut statuses = 0;
    let request = MultiCodeSaleRequest::new(5_000, "8", "0").unwrap();
    let sale = pos
        .multicode_sale_with_status(&request, |_| statuses += 1)
        .unwrap();
    assert!(sale.is_approved());
    assert_eq!(statuses, 4);
    assert_eq!(sale.commerce_provider_code.as_deref(), Some("597029414300"));

    let last = pos.multicode_last_sale(true).unwrap();
    assert_eq!(last.sale.function_code, "0281");
    assert_eq!(last.sale.ticket.as_deref(), Some("000008"));
    assert_eq!(last.sale.voucher_lines().map(|lines| lines.len()), Some(3));

    let detail = pos
        .multicode_sales_detail(SalesDetailMode::SendToHost)
        .unwrap();
    assert_eq!(detail.len(), 2);
    assert_eq!(
        detail[0].commerce_provider_code.as_deref(),
        Some("597012345678")
    );
    assert_eq!(
        pos.multicode_sales_detail(SalesDetailMode::PrintOnPos)
            .unwrap(),
        []
    );

    let sent = pos
        .link_mut()
        .transport_mut()
        .sim()
        .received_payloads()
        .to_vec();
    assert_eq!(sent[0], "0270|000012000|000007||0|0|597012345678");
    assert_eq!(sent[1], "0270|000005000|000008||0|1|0");
}

#[test]
fn close_clears_the_period() {
    let mut pos = pos();
    pos.sale(&sale_request(12_000, "1")).unwrap();
    assert!(pos.close().unwrap().is_approved());
    assert_eq!(pos.totals().unwrap().tx_count, Some(0));
}

#[test]
fn client_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<PosIntegrado<SimTransport>>();
    assert_send::<Pos>();
}
