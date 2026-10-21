//! Link layer against a scripted POS with simulated time (QA-03): ACK/NAK retries,
//! intermediate messages, bad LRC, garbage, timeouts and sales detail lists.

use std::time::Duration;

use transbank_pos::protocol::{ACK, NAK, encode_frame};
use transbank_pos::transport::{ManualClock, MemoryTransport};
use transbank_pos::{Error, IntermediateStatus, Link, LinkConfig, ResponseCode};

const SALE_REQUEST: &str = "0200|000012000|ABC123||0|1";
const SALE_RESPONSE: &str = "0210|00|597029414300|IT750050|ABC123|757752|12000|03|3334|9480|000135|CR|22102025|0000000000000000000|MC|22102025|114016|7|500|";

/// Encoded frame; an invalid payload yields an empty frame (and a failing test).
fn frame(payload: &str) -> Vec<u8> {
    encode_frame(payload).unwrap_or_default()
}

fn corrupted(payload: &str) -> Vec<u8> {
    let mut bytes = frame(payload);
    if let Some(last) = bytes.last_mut() {
        *last ^= 0xFF;
    }
    bytes
}

fn link(transport: MemoryTransport, clock: &ManualClock) -> Link<MemoryTransport, ManualClock> {
    Link::with_clock(transport, LinkConfig::default(), clock.clone())
}

fn count_occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn no_status() -> impl FnMut(IntermediateStatus) {
    |_| {}
}

#[test]
fn poll_writes_exact_frame_and_accepts_ack() {
    let clock = ManualClock::new();
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&[0x02, 0x30, 0x31, 0x30, 0x30, 0x03, 0x02])
        .then_reply(&[ACK]);
    let mut link = link(transport, &clock);
    link.send_command("0100").unwrap();
    assert!(link.transport_mut().is_finished());
    assert_eq!(clock.now(), Duration::ZERO);
}

#[test]
fn no_ack_retries_three_times() {
    let clock = ManualClock::new();
    let poll = frame("0100");
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&poll)
        .expect_write(&poll)
        .expect_write(&poll);
    let mut link = link(transport, &clock);
    assert!(matches!(
        link.send_command("0100"),
        Err(Error::NoAck {
            attempts: 3,
            naks: 0
        })
    ));
    assert_eq!(clock.now(), Duration::from_secs(6));
    assert_eq!(link.transport_mut().written(), poll.repeat(3));
}

#[test]
fn nak_nak_ack_succeeds_on_third_attempt() {
    let clock = ManualClock::new();
    let poll = frame("0100");
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&poll)
        .then_reply(&[NAK])
        .expect_write(&poll)
        .then_reply(&[NAK])
        .expect_write(&poll)
        .then_reply(&[ACK]);
    let mut link = link(transport, &clock);
    link.send_command("0100").unwrap();
    assert!(link.transport_mut().is_finished());
    assert_eq!(clock.now(), Duration::ZERO);
}

#[test]
fn three_naks_fail_with_nak_count() {
    let clock = ManualClock::new();
    let poll = frame("0100");
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&poll)
        .then_reply(&[NAK])
        .expect_write(&poll)
        .then_reply(&[NAK])
        .expect_write(&poll)
        .then_reply(&[NAK]);
    let mut link = link(transport, &clock);
    assert!(matches!(
        link.send_command("0100"),
        Err(Error::NoAck {
            attempts: 3,
            naks: 3
        })
    ));
}

#[test]
fn sale_with_intermediate_messages() {
    let clock = ManualClock::new();
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&[ACK])
        .then_reply(&frame("0900|78"))
        .expect_write(&[ACK])
        .then_reply(&frame("0900|81"))
        .expect_write(&[ACK])
        .then_reply(&frame(SALE_RESPONSE))
        .expect_write(&[ACK]);
    let mut link = link(transport, &clock);
    let mut statuses = Vec::new();
    let response = link
        .request(SALE_REQUEST, "0210", &mut |status| statuses.push(status))
        .unwrap();
    assert_eq!(response, SALE_RESPONSE);
    assert_eq!(
        statuses,
        [
            IntermediateStatus::Code(ResponseCode::WaitingForCard),
            IntermediateStatus::Code(ResponseCode::RequestingPin),
        ]
    );
    let mut expected_written = frame(SALE_REQUEST);
    expected_written.extend([ACK, ACK, ACK]);
    assert_eq!(link.transport_mut().written(), expected_written);
    assert!(link.transport_mut().is_finished());
}

#[test]
fn bad_lrc_is_nakked_then_good_frame_accepted() {
    let clock = ManualClock::new();
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame("0800"))
        .then_reply(&[ACK])
        .then_reply(&corrupted("0810|00|597029414300|IT750050"))
        .expect_write(&[NAK])
        .then_reply(&frame("0810|00|597029414300|IT750050"))
        .expect_write(&[ACK]);
    let mut link = link(transport, &clock);
    let response = link.request("0800", "0810", &mut no_status()).unwrap();
    assert_eq!(response, "0810|00|597029414300|IT750050");
    assert!(link.transport_mut().is_finished());
}

#[test]
fn more_than_three_bad_lrc_fail() {
    let clock = ManualClock::new();
    let bad = corrupted("0810|00");
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame("0800"))
        .then_reply(&[ACK])
        .then_reply(&bad)
        .expect_write(&[NAK])
        .then_reply(&bad)
        .expect_write(&[NAK])
        .then_reply(&bad)
        .expect_write(&[NAK])
        .then_reply(&bad);
    let mut link = link(transport, &clock);
    assert!(matches!(
        link.request("0800", "0810", &mut no_status()),
        Err(Error::BadLrc { count: 4 })
    ));
    assert!(link.transport_mut().is_finished());
}

#[test]
fn garbage_is_ignored() {
    let clock = ManualClock::new();
    let mut reply = b"\xFF\x00zz".to_vec();
    reply.push(ACK);
    reply.extend(b"junk\x03");
    reply.extend(frame("0710|00|002|15000||"));
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame("0700||"))
        .then_reply(&reply)
        .expect_write(&[ACK]);
    let mut link = link(transport, &clock);
    let response = link.request("0700||", "0710", &mut no_status()).unwrap();
    assert_eq!(response, "0710|00|002|15000||");
}

#[test]
fn unexpected_function_code_is_an_error_after_ack() {
    let clock = ManualClock::new();
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&[ACK])
        .then_reply(&frame("0810|00"))
        .expect_write(&[ACK]);
    let mut link = link(transport, &clock);
    let err = link
        .request(SALE_REQUEST, "0210", &mut no_status())
        .unwrap_err();
    assert!(
        matches!(err, Error::UnexpectedResponse { expected: "0210", ref received, ref payload }
            if received == "0810" && payload == "0810|00"),
        "{err:?}"
    );
    assert!(link.transport_mut().is_finished());
}

/// LINK-03: an acknowledged sale without response is never written again.
#[test]
fn acknowledged_sale_without_response_is_not_resent() {
    let clock = ManualClock::new();
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&[ACK]);
    let mut link = link(transport, &clock);
    assert!(matches!(
        link.request(SALE_REQUEST, "0210", &mut no_status()),
        Err(Error::ResponseTimeout { waited }) if waited == Duration::from_secs(150)
    ));
    assert_eq!(clock.now(), Duration::from_secs(150));
    let written = link.transport_mut().written().to_vec();
    assert_eq!(count_occurrences(&written, &frame(SALE_REQUEST)), 1);
    assert_eq!(written, frame(SALE_REQUEST));
}

/// A frame arriving instead of the ACK is an implicit ACK: the sale is not resent.
#[test]
fn response_without_ack_is_an_implicit_ack() {
    let clock = ManualClock::new();
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&frame(SALE_RESPONSE))
        .expect_write(&[ACK]);
    let mut link = link(transport, &clock);
    let response = link
        .request(SALE_REQUEST, "0210", &mut no_status())
        .unwrap();
    assert_eq!(response, SALE_RESPONSE);
    assert_eq!(
        count_occurrences(link.transport_mut().written(), &frame(SALE_REQUEST)),
        1
    );
}

#[test]
fn intermediate_messages_do_not_extend_the_deadline() {
    let clock = ManualClock::new();
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&[ACK])
        .then_reply_after(Duration::from_secs(100), &frame("0900|78"))
        .expect_write(&[ACK])
        .then_reply_after(Duration::from_secs(100), &frame("0900|81"));
    let mut link = link(transport, &clock);
    let mut statuses = 0;
    let result = link.request(SALE_REQUEST, "0210", &mut |_| statuses += 1);
    assert!(matches!(result, Err(Error::ResponseTimeout { .. })));
    assert_eq!(statuses, 1);
    assert_eq!(clock.now(), Duration::from_secs(150));
}

fn detail(n: u32) -> String {
    format!(
        "0261|00|597029414300|IT750050|{n:06}|{:06}|1000|9480|{n:06}|CR|22102025|0000000000000000000|MC|22102025|114016|1|0|0|0",
        700_000 + n
    )
}

const END: &str = "0261|00|597029414300|IT750050||||||||||||||||";

fn list_script(clock: &ManualClock, payloads: &[String]) -> MemoryTransport {
    let mut transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame("0260|1|"))
        .then_reply(&[ACK]);
    for payload in payloads {
        transport = transport.then_reply(&frame(payload)).expect_write(&[ACK]);
    }
    transport
}

#[test]
fn detail_list_with_two_terminators() {
    let clock = ManualClock::new();
    let mut payloads: Vec<String> = (1..=3).map(detail).collect();
    payloads.push(END.to_owned());
    payloads.push(END.to_owned());
    let mut link = link(list_script(&clock, &payloads), &clock);
    let entries = link.request_list("0260|1|", "0261").unwrap();
    assert_eq!(entries, [detail(1), detail(2), detail(3)]);
    assert!(link.transport_mut().is_finished());
    assert_eq!(clock.now(), Duration::from_millis(500));
}

#[test]
fn detail_list_with_one_terminator_waits_the_tail() {
    let clock = ManualClock::new();
    let mut payloads: Vec<String> = (1..=3).map(detail).collect();
    payloads.push(END.to_owned());
    let mut link = link(list_script(&clock, &payloads), &clock);
    let entries = link.request_list("0260|1|", "0261").unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(clock.now(), Duration::from_millis(500));
}

#[test]
fn detail_list_without_sales() {
    let clock = ManualClock::new();
    let payloads = ["0261|11||||||||||||||||||".to_owned()];
    let mut link = link(list_script(&clock, &payloads), &clock);
    assert_eq!(
        link.request_list("0260|1|", "0261").unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn detail_list_is_bounded() {
    let clock = ManualClock::new();
    let payloads: Vec<String> = (1..=10_001).map(detail).collect();
    let mut link = link(list_script(&clock, &payloads), &clock);
    assert!(matches!(
        link.request_list("0260|1|", "0261"),
        Err(Error::TooManyFrames { max: 10_000 })
    ));
}

#[test]
fn link_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Link<MemoryTransport, ManualClock>>();
    assert_send::<Link<MemoryTransport>>();
}
