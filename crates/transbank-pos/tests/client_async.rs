//! Asynchronous typed client tests (`#![cfg(feature = "tokio")]`): tests [`AsyncPosIntegrado`]
//! against [`AsyncSimStream`] and local TCP server.

#![cfg(feature = "tokio")]

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use transbank_pos::tokio::AsyncPosIntegrado;
use transbank_pos::{
    Error, IntermediateStatus, LinkConfig, MultiCodeSaleRequest, OperationNumber, ResponseCode,
    SaleRequest, SalesDetailMode,
};
use transbank_pos_sim::{AsyncSimStream, PosSimulator, Scenario, TerminalKind};

fn test_client() -> AsyncPosIntegrado<AsyncSimStream> {
    let sim = PosSimulator::new(TerminalKind::Integrado);
    let stream = AsyncSimStream::new(sim);
    let mut config = LinkConfig::default();
    config.ack_timeout = Duration::from_millis(50);
    config.response_timeout = Duration::from_millis(200);
    config.detail_tail = Duration::from_millis(50);
    config.max_attempts = 3;
    config.max_bad_lrc = 3;
    AsyncPosIntegrado::with_config(stream, config)
}

#[tokio::test(start_paused = true)]
async fn every_method_of_client() {
    let mut pos = test_client();

    // 1. poll
    pos.poll().await.unwrap();

    // 2. load_keys
    let load_res = pos.load_keys().await.unwrap();
    assert!(load_res.is_approved());

    // 3. sale (approved)
    let req = SaleRequest::new(12_000, "ABC123").unwrap();
    let sale = pos.sale(&req).await.unwrap();
    assert!(sale.is_approved());
    assert_eq!(sale.amount, Some(12_000));
    assert_eq!(sale.ticket.as_deref(), Some("ABC123"));

    // 4. sale_with_status
    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::Intermediate(vec![78, 81]));
    let mut statuses = Vec::new();
    let sale_status = pos
        .sale_with_status(&req, |s| statuses.push(s))
        .await
        .unwrap();
    assert!(sale_status.is_approved());
    assert_eq!(
        statuses,
        [
            IntermediateStatus::Code(ResponseCode::WaitingForCard),
            IntermediateStatus::Code(ResponseCode::RequestingPin),
        ]
    );

    // 5. multicode_sale
    let multi_req = MultiCodeSaleRequest::new(10_000, "TICKET", "597029414300").unwrap();
    let multi_sale = pos.multicode_sale(&multi_req).await.unwrap();
    assert!(multi_sale.sale.is_approved());

    // 6. multicode_sale_with_status
    let mut multi_statuses = Vec::new();
    let multi_status_sale = pos
        .multicode_sale_with_status(&multi_req, |s| multi_statuses.push(s))
        .await
        .unwrap();
    assert!(multi_status_sale.sale.is_approved());

    // 7. last_sale
    let last = pos.last_sale().await.unwrap();
    assert!(last.is_approved());

    // 8. multicode_last_sale
    let multi_last = pos.multicode_last_sale(false).await.unwrap();
    assert!(multi_last.sale.is_approved());

    // 9. totals
    let totals = pos.totals().await.unwrap();
    assert!(totals.is_approved());
    assert_eq!(totals.tx_count, Some(4));

    // 10. sales_detail
    let printed = pos.sales_detail(SalesDetailMode::PrintOnPos).await.unwrap();
    assert!(printed.is_empty());
    let detail_entries = pos.sales_detail(SalesDetailMode::SendToHost).await.unwrap();
    assert_eq!(detail_entries.len(), 4);

    // 11. multicode_sales_detail
    let multi_printed = pos
        .multicode_sales_detail(SalesDetailMode::PrintOnPos)
        .await
        .unwrap();
    assert!(multi_printed.is_empty());
    let multi_detail = pos
        .multicode_sales_detail(SalesDetailMode::SendToHost)
        .await
        .unwrap();
    assert_eq!(multi_detail.len(), 4);

    // 12. refund
    let refund = pos.refund(OperationNumber::new(1).unwrap()).await.unwrap();
    assert!(refund.is_approved());

    // 13. close
    let close = pos.close().await.unwrap();
    assert!(close.is_approved());

    // 14. set_normal_mode
    pos.set_normal_mode().await.unwrap();

    // into_inner
    let stream = pos.into_inner();
    assert_eq!(stream.sim().sales().len(), 0);
}

#[tokio::test(start_paused = true)]
async fn sale_approved_and_rejected() {
    let mut pos = test_client();

    // Approved
    let req = SaleRequest::new(15_000, "APP001").unwrap();
    let approved = pos.sale(&req).await.unwrap();
    assert!(approved.is_approved());
    assert_eq!(approved.response_code, ResponseCode::Approved);

    // Rejected
    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::RejectNextSale(1));
    let req2 = SaleRequest::new(15_000, "REJ001").unwrap();
    let rejected = pos.sale(&req2).await.unwrap();
    assert!(!rejected.is_approved());
    assert_eq!(rejected.response_code, ResponseCode::Rejected);
}

#[tokio::test(start_paused = true)]
async fn sale_no_response_times_out_and_is_resolved_by_last_sale() {
    let mut pos = test_client();

    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::NoResponse);
    let req = SaleRequest::new(20_000, "TMO001").unwrap();
    let err = pos.sale(&req).await.unwrap_err();
    assert!(matches!(err, Error::ResponseTimeout { .. }));

    // LINK-03: Exactly one 0200 was received by the simulator
    let received = pos.link_mut().stream_mut().sim().received_payloads();
    let count_0200 = received.iter().filter(|p| p.starts_with("0200")).count();
    assert_eq!(count_0200, 1);

    // Resolve via last_sale
    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::Approve);
    let last = pos.last_sale().await.unwrap();
    assert!(last.is_approved());
    assert_eq!(last.ticket.as_deref(), Some("TMO001"));
    assert_eq!(last.amount, Some(20_000));
}

#[tokio::test(start_paused = true)]
async fn refund_no_response_times_out_and_receives_exactly_one_frame() {
    let mut pos = test_client();

    // Register a credit sale first (even amount = credit)
    let req = SaleRequest::new(10_000, "REF001").unwrap();
    pos.sale(&req).await.unwrap();

    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::NoResponse);
    let err = pos
        .refund(OperationNumber::new(1).unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::ResponseTimeout { .. }));

    // Exactly one 1200 frame was received by the simulator
    let received = pos.link_mut().stream_mut().sim().received_payloads();
    let count_1200 = received.iter().filter(|p| p.starts_with("1200")).count();
    assert_eq!(count_1200, 1);
}

#[tokio::test(start_paused = true)]
async fn failure_scenarios() {
    let mut pos = test_client();

    // CorruptLrc: recovers after NAK
    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::CorruptLrc(2));
    assert!(pos.load_keys().await.unwrap().is_approved());

    // NakNext: command retransmitted on NAK
    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::NakNext(2));
    pos.poll().await.unwrap();

    // DropAcks: command retransmitted on ACK timeout
    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::DropAcks(2));
    pos.poll().await.unwrap();

    // WrongFunctionCode: unexpected response code triggers Error::UnexpectedResponse
    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::WrongFunctionCode("0810"));
    let req = SaleRequest::new(10_000, "WRG001").unwrap();
    let err = pos.sale(&req).await.unwrap_err();
    assert!(matches!(err, Error::UnexpectedResponse { .. }));
}

#[tokio::test(start_paused = true)]
async fn sales_detail_with_and_without_sales() {
    let mut pos = test_client();

    // Without sales
    let entries = pos.sales_detail(SalesDetailMode::SendToHost).await.unwrap();
    assert!(entries.is_empty());

    // Make 2 sales
    pos.sale(&SaleRequest::new(10_000, "D01").unwrap())
        .await
        .unwrap();
    pos.sale(&SaleRequest::new(11_000, "D02").unwrap())
        .await
        .unwrap();

    let entries = pos.sales_detail(SalesDetailMode::SendToHost).await.unwrap();
    assert_eq!(entries.len(), 2);
}

#[tokio::test]
async fn connect_tcp_local_server() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 7];
        socket.read_exact(&mut buf).await.unwrap();
        // Respond with ACK to poll command
        socket.write_all(&[0x06]).await.unwrap();
        socket.flush().await.unwrap();
    });

    let mut client = AsyncPosIntegrado::connect_tcp(addr).await.unwrap();
    client.poll().await.unwrap();
    server_task.await.unwrap();
}

#[test]
fn client_and_futures_are_send() {
    fn assert_send<T: Send>(_: &T) {}
    let client: Option<AsyncPosIntegrado<tokio::net::TcpStream>> = None;
    if let Some(mut c) = client {
        assert_send(&c);
        let req = SaleRequest::new(1_000, "T").unwrap();
        let sale_fut = c.sale(&req);
        assert_send(&sale_fut);
        drop(sale_fut);
        let sale_status_fut = c.sale_with_status(&req, |_| {});
        assert_send(&sale_status_fut);
        drop(sale_status_fut);
    }
}

#[tokio::test(start_paused = true)]
async fn sale_corrupt_lrc_exhausted_fails_with_bad_lrc_and_single_request() {
    let mut pos = test_client();
    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::CorruptLrc(4));
    let req = SaleRequest::new(10_000, "BADLRC").unwrap();
    let err = pos.sale(&req).await.unwrap_err();
    assert!(matches!(err, Error::BadLrc { count: 4 }));

    // LINK-03: exactly one 0200 was received by the simulator
    let received = pos.link_mut().stream_mut().sim().received_payloads();
    let count_0200 = received.iter().filter(|p| p.starts_with("0200")).count();
    assert_eq!(count_0200, 1);
}

#[tokio::test(start_paused = true)]
async fn sale_recovers_from_corrupt_lrc_and_approves() {
    let mut pos = test_client();
    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::CorruptLrc(2));
    let req = SaleRequest::new(10_000, "CORR02").unwrap();
    let res = pos.sale(&req).await.unwrap();
    assert!(res.is_approved());
    assert_eq!(pos.link_mut().stream_mut().sim().sales().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn sale_recovers_from_nak_and_approves() {
    let mut pos = test_client();
    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::NakNext(2));
    let req = SaleRequest::new(10_000, "NAK002").unwrap();
    let res = pos.sale(&req).await.unwrap();
    assert!(res.is_approved());
    assert_eq!(pos.link_mut().stream_mut().sim().sales().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn sale_recovers_from_drop_acks_and_approves() {
    let mut pos = test_client();
    pos.link_mut()
        .stream_mut()
        .sim_mut()
        .set_scenario(Scenario::DropAcks(2));
    let req = SaleRequest::new(10_000, "DROP02").unwrap();
    let res = pos.sale(&req).await.unwrap();
    assert!(res.is_approved());
    assert_eq!(pos.link_mut().stream_mut().sim().sales().len(), 1);
}

#[cfg(feature = "tokio-serial")]
#[test]
fn open_serial_outside_runtime_returns_transport_error() {
    let res = AsyncPosIntegrado::open_serial("/nonexistent/tbk-pos-test", 115_200);
    assert!(matches!(res, Err(Error::Transport(_))));
}
