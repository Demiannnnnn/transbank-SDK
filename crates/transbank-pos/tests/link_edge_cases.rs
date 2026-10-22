//! Link edge cases: byte-by-byte delivery, LRC values equal to control bytes in
//! responses, transport failures, configuration limits and garbage around lists.

use std::time::Duration;

use proptest::prelude::*;
use transbank_pos::protocol::{ACK, NAK, encode_frame, lrc};
use transbank_pos::transport::{ManualClock, MemoryTransport};
use transbank_pos::{Error, Link, LinkConfig, Result, Transport};

/// Encoded frame; an invalid payload yields an empty frame (and a failing test).
fn frame(payload: &str) -> Vec<u8> {
    encode_frame(payload).unwrap_or_default()
}

/// Delivers at most `chunk` bytes per read, like a slow serial port.
struct Trickle<T> {
    inner: T,
    chunk: usize,
    reads: usize,
}

impl<T: Transport> Transport for Trickle<T> {
    fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
        self.inner.write_all(bytes)
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        self.reads += 1;
        let len = buf.len().min(self.chunk);
        let slice = buf.get_mut(..len).unwrap_or_default();
        self.inner.read_timeout(slice, timeout)
    }
}

/// Fails every read with the given I/O error kind.
struct Broken(std::io::ErrorKind);

impl Transport for Broken {
    fn write_all(&mut self, _bytes: &[u8]) -> Result<()> {
        Ok(())
    }

    fn read_timeout(&mut self, _buf: &mut [u8], _timeout: Duration) -> Result<usize> {
        Err(Error::Io(std::io::Error::from(self.0)))
    }
}

fn sale_script(clock: &ManualClock, response: &str) -> MemoryTransport {
    MemoryTransport::new(clock.clone())
        .expect_write(&frame("0250|"))
        .then_reply(&[ACK])
        .then_reply(&frame(response))
        .expect_write(&[ACK])
}

#[test]
fn byte_by_byte_delivery() {
    let clock = ManualClock::new();
    let response = "0260|00|597029414300|IT750050|ABC123|757752|12000";
    let transport = Trickle {
        inner: sale_script(&clock, response),
        chunk: 1,
        reads: 0,
    };
    let mut link = Link::with_clock(transport, LinkConfig::default(), clock.clone());
    assert_eq!(
        link.request("0250|", "0260", &mut |_| {}).unwrap(),
        response
    );
    assert!(link.transport_mut().reads > frame(response).len());
    assert_eq!(clock.now(), Duration::ZERO);
}

/// A `0260|00|<n><c>` payload whose LRC is `target`: the LRC is a XOR, so the last
/// character is solved as `lrc(prefix) ^ target`.
fn payload_with_lrc(target: u8) -> String {
    (0..1_000)
        .find_map(|n| {
            let prefix = format!("0260|00|{n}");
            // lrc(prefix + c) == lrc(prefix) ^ c
            let c = lrc(prefix.as_bytes()) ^ target;
            ((0x20..0x7F).contains(&c) && c != b'|').then(|| format!("{prefix}{}", char::from(c)))
        })
        .unwrap_or_default()
}

#[test]
fn responses_whose_lrc_is_a_control_byte() {
    for target in [0x02, 0x03, ACK, NAK] {
        let payload = payload_with_lrc(target);
        assert_eq!(frame(&payload).last(), Some(&target));
        let clock = ManualClock::new();
        let mut link =
            Link::with_clock(sale_script(&clock, &payload), LinkConfig::default(), clock);
        assert_eq!(link.request("0250|", "0260", &mut |_| {}).unwrap(), payload);
        assert!(link.transport_mut().is_finished());
    }
}

#[test]
fn stale_input_is_discarded_before_sending() {
    let clock = ManualClock::new();
    let mut transport = sale_script(&clock, "0260|00|1");
    // A stale response from a previous command must not be taken as this one.
    transport.push_incoming(&frame("0260|00|OLD"));
    transport.push_incoming(&[ACK]);
    let mut link = Link::with_clock(transport, LinkConfig::default(), clock);
    assert_eq!(
        link.request("0250|", "0260", &mut |_| {}).unwrap(),
        "0260|00|1"
    );
}

#[test]
fn io_errors_while_waiting_propagate() {
    for kind in [
        std::io::ErrorKind::UnexpectedEof,
        std::io::ErrorKind::BrokenPipe,
    ] {
        let mut link = Link::new(Broken(kind), LinkConfig::default());
        let err = link.send_command("0100").unwrap_err();
        assert!(
            matches!(err, Error::Io(ref e) if e.kind() == kind),
            "{err:?}"
        );
    }
}

#[test]
fn write_errors_propagate() {
    let clock = ManualClock::new();
    let transport = MemoryTransport::new(clock.clone()).expect_write(&frame("0300"));
    let mut link = Link::with_clock(transport, LinkConfig::default(), clock);
    assert!(matches!(link.send_command("0100"), Err(Error::Io(_))));
}

#[test]
fn invalid_payloads_are_never_written() {
    let clock = ManualClock::new();
    let mut link = Link::with_clock(
        MemoryTransport::new(clock.clone()),
        LinkConfig::default(),
        clock,
    );
    assert!(matches!(
        link.send_command(""),
        Err(Error::InvalidArgument { .. })
    ));
    assert!(matches!(
        link.request("02\u{3}00", "0210", &mut |_| {}),
        Err(Error::InvalidArgument { .. })
    ));
    assert!(matches!(
        link.request_list(&"1".repeat(5000), "0261"),
        Err(Error::Overflow { .. })
    ));
    assert_eq!(link.transport_mut().written(), b"");
}

#[test]
fn zero_attempts_still_sends_once() {
    let clock = ManualClock::new();
    let mut config = LinkConfig::default();
    config.max_attempts = 0;
    let transport = MemoryTransport::new(clock.clone()).expect_write(&frame("0100"));
    let mut link = Link::with_clock(transport, config, clock);
    assert!(matches!(
        link.send_command("0100"),
        Err(Error::NoAck {
            attempts: 1,
            naks: 0
        })
    ));
}

#[test]
fn custom_timeouts_are_respected() {
    let clock = ManualClock::new();
    let mut config = LinkConfig::default();
    config.ack_timeout = Duration::from_millis(300);
    config.max_attempts = 5;
    config.response_timeout = Duration::from_secs(7);
    let poll = frame("0100");
    let mut transport = MemoryTransport::new(clock.clone());
    for _ in 0..5 {
        transport = transport.expect_write(&poll);
    }
    let mut link = Link::with_clock(transport, config.clone(), clock.clone());
    assert!(matches!(
        link.send_command("0100"),
        Err(Error::NoAck { attempts: 5, .. })
    ));
    assert_eq!(clock.now(), Duration::from_millis(1500));
    assert_eq!(link.config(), &config);

    let clock = ManualClock::new();
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame("0700||"))
        .then_reply(&[ACK]);
    let mut link = Link::with_clock(transport, config, clock.clone());
    assert!(matches!(
        link.request("0700||", "0710", &mut |_| {}),
        Err(Error::ResponseTimeout { waited }) if waited == Duration::from_secs(7)
    ));
    assert_eq!(clock.now(), Duration::from_secs(7));
}

#[test]
fn bad_lrc_limit_is_configurable() {
    let clock = ManualClock::new();
    let mut config = LinkConfig::default();
    config.max_bad_lrc = 0;
    let mut bad = frame("0710|00");
    if let Some(last) = bad.last_mut() {
        *last ^= 0xFF;
    }
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame("0700||"))
        .then_reply(&[ACK])
        .then_reply(&bad);
    let mut link = Link::with_clock(transport, config, clock);
    assert!(matches!(
        link.request("0700||", "0710", &mut |_| {}),
        Err(Error::BadLrc { count: 1 })
    ));
}

#[test]
fn bad_lrc_counter_resets_after_a_good_frame() {
    let clock = ManualClock::new();
    let mut bad = frame("0900|78");
    if let Some(last) = bad.last_mut() {
        *last ^= 0xFF;
    }
    let mut transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame("0200|000001000|1||0|1"))
        .then_reply(&[ACK]);
    // 3 bad + 1 good intermediate, twice: never 4 bad in a row.
    for _ in 0..2 {
        for _ in 0..3 {
            transport = transport.then_reply(&bad).expect_write(&[NAK]);
        }
        transport = transport.then_reply(&frame("0900|78")).expect_write(&[ACK]);
    }
    transport = transport
        .then_reply(&frame("0210|00|1"))
        .expect_write(&[ACK]);
    let mut link = Link::with_clock(transport, LinkConfig::default(), clock);
    let mut statuses = 0;
    let response = link
        .request("0200|000001000|1||0|1", "0210", &mut |_| statuses += 1)
        .unwrap();
    assert_eq!(response, "0210|00|1");
    assert_eq!(statuses, 2);
}

#[test]
fn list_with_intermediate_garbage_and_bad_lrc_in_the_tail() {
    let clock = ManualClock::new();
    let entry = "0261|00|1|T|1|123456|1000";
    let end = "0261|00|1|T||";
    let mut bad_end = frame(end);
    if let Some(last) = bad_end.last_mut() {
        *last ^= 0xFF;
    }
    let mut garbage_then_entry = b"\x00junk".to_vec();
    garbage_then_entry.extend(frame(entry));
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame("0260|1|"))
        .then_reply(&[ACK])
        .then_reply(&frame("0900|82"))
        .expect_write(&[ACK])
        .then_reply(&garbage_then_entry)
        .expect_write(&[ACK])
        .then_reply(&frame(end))
        .expect_write(&[ACK])
        .then_reply(&bad_end)
        .expect_write(&[NAK])
        .then_reply(&frame(end))
        .expect_write(&[ACK]);
    let mut link = Link::with_clock(transport, LinkConfig::default(), clock.clone());
    assert_eq!(link.request_list("0260|1|", "0261").unwrap(), [entry]);
    assert!(link.transport_mut().is_finished());
    assert_eq!(clock.now(), Duration::from_millis(500));
}

#[test]
fn wrong_frame_in_list_tail_is_unexpected() {
    let clock = ManualClock::new();
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame("0260|1|"))
        .then_reply(&[ACK])
        .then_reply(&frame("0261|11||||"))
        .expect_write(&[ACK])
        .then_reply(&frame("0810|00"))
        .expect_write(&[ACK]);
    let mut link = Link::with_clock(transport, LinkConfig::default(), clock);
    assert!(matches!(
        link.request_list("0260|1|", "0261"),
        Err(Error::UnexpectedResponse {
            expected: "0261",
            ..
        })
    ));
}

#[test]
fn list_entry_after_the_first_terminator_is_kept() {
    let clock = ManualClock::new();
    let late = "0261|00|1|T|2|654321|2000";
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame("0260|1|"))
        .then_reply(&[ACK])
        .then_reply(&frame("0261|00|1|T||"))
        .expect_write(&[ACK])
        .then_reply_after(Duration::from_millis(400), &frame(late))
        .expect_write(&[ACK]);
    let mut link = Link::with_clock(transport, LinkConfig::default(), clock.clone());
    assert_eq!(link.request_list("0260|1|", "0261").unwrap(), [late]);
    // 400 ms until the late entry, then a full tail of silence.
    assert_eq!(clock.now(), Duration::from_millis(900));
}

#[test]
fn into_inner_returns_the_transport() {
    let clock = ManualClock::new();
    let transport = MemoryTransport::new(clock.clone())
        .expect_write(&frame("0100"))
        .then_reply(&[ACK]);
    let mut link = Link::with_clock(transport, LinkConfig::default(), clock);
    link.send_command("0100").unwrap();
    assert_eq!(link.into_inner().written(), frame("0100"));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Whatever the chunk size and the garbage before the ACK and between frames,
    /// the exchange succeeds and the response is the same.
    #[test]
    fn any_chunking_and_garbage(
        chunk in 1usize..32,
        before_ack in prop::collection::vec(0x20u8..0x7F, 0..20),
        between in prop::collection::vec(0x20u8..0x7F, 0..20),
    ) {
        let clock = ManualClock::new();
        let response = "0710|00|003|45000||";
        let mut reply = before_ack.clone();
        reply.push(ACK);
        reply.extend(&between);
        reply.extend(frame("0900|79"));
        let transport = MemoryTransport::new(clock.clone())
            .expect_write(&frame("0700||"))
            .then_reply(&reply)
            .expect_write(&[ACK])
            .then_reply(&between)
            .then_reply(&frame(response))
            .expect_write(&[ACK]);
        let transport = Trickle { inner: transport, chunk, reads: 0 };
        let mut link = Link::with_clock(transport, LinkConfig::default(), clock);
        let mut statuses = 0;
        let result = link.request("0700||", "0710", &mut |_| statuses += 1);
        prop_assert_eq!(result.unwrap(), response);
        prop_assert_eq!(statuses, 1);
    }
}
