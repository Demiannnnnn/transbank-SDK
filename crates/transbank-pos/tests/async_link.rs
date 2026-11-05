//! Asynchronous link layer tests (`#![cfg(feature = "tokio")]`): translates all test cases
//! from `tests/link.rs` and `tests/link_edge_cases.rs` to [`AsyncLink`], plus the mandatory
//! traps a–f with simulated time (`#[tokio::test(start_paused = true)]`, QA-03).

#![cfg(feature = "tokio")]

use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use transbank_pos::protocol::{ACK, NAK, encode_frame, lrc};
use transbank_pos::tokio::AsyncLink;
use transbank_pos::{Error, IntermediateStatus, LinkConfig, ResponseCode};

const SALE_REQUEST: &str = "0200|000012000|ABC123||0|1";
const SALE_RESPONSE: &str = "0210|00|597029414300|IT750050|ABC123|757752|12000|03|3334|9480|000135|CR|22102025|0000000000000000000|MC|22102025|114016|7|500|";

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

fn count_occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

#[derive(Clone)]
enum ScriptItem {
    Write(Vec<u8>),
    Reply(Duration, Vec<u8>),
}

/// Scripted asynchronous in-memory stream for testing `AsyncLink` deterministically.
struct AsyncScriptedStream {
    script: VecDeque<ScriptItem>,
    incoming: VecDeque<u8>,
    written: Vec<u8>,
    flush_count: usize,
    chunk_size: usize,
    closed_eof: bool,
    read_waker: Option<Waker>,
    deliver_at: Option<tokio::time::Instant>,
    pending_reply: Option<Vec<u8>>,
    active_sleep: Option<Pin<Box<tokio::time::Sleep>>>,
    read_error: Option<io::ErrorKind>,
    write_error: Option<io::ErrorKind>,
}

impl AsyncScriptedStream {
    fn new() -> Self {
        Self {
            script: VecDeque::new(),
            incoming: VecDeque::new(),
            written: Vec::new(),
            flush_count: 0,
            chunk_size: 512,
            closed_eof: false,
            read_waker: None,
            deliver_at: None,
            pending_reply: None,
            active_sleep: None,
            read_error: None,
            write_error: None,
        }
    }

    fn expect_write(mut self, bytes: &[u8]) -> Self {
        self.script.push_back(ScriptItem::Write(bytes.to_vec()));
        self
    }

    fn then_reply(mut self, bytes: &[u8]) -> Self {
        self.script
            .push_back(ScriptItem::Reply(Duration::ZERO, bytes.to_vec()));
        self.advance_script();
        self
    }

    fn then_reply_after(mut self, after: Duration, bytes: &[u8]) -> Self {
        self.script
            .push_back(ScriptItem::Reply(after, bytes.to_vec()));
        self.advance_script();
        self
    }

    fn push_incoming(&mut self, bytes: &[u8]) {
        self.incoming.extend(bytes);
        if let Some(waker) = self.read_waker.take() {
            waker.wake();
        }
    }

    fn set_chunk_size(&mut self, size: usize) {
        self.chunk_size = size;
    }

    fn close(&mut self) {
        self.closed_eof = true;
        if let Some(waker) = self.read_waker.take() {
            waker.wake();
        }
    }

    fn set_read_error(&mut self, kind: io::ErrorKind) {
        self.read_error = Some(kind);
    }

    fn set_write_error(&mut self, kind: io::ErrorKind) {
        self.write_error = Some(kind);
    }

    fn written(&self) -> &[u8] {
        &self.written
    }

    fn flush_count(&self) -> usize {
        self.flush_count
    }

    fn is_finished(&self) -> bool {
        self.script.is_empty()
            && self.incoming.is_empty()
            && self.deliver_at.is_none()
            && self.pending_reply.is_none()
    }

    fn advance_script(&mut self) {
        while self.deliver_at.is_none() {
            match self.script.front() {
                Some(ScriptItem::Reply(delay, _)) => {
                    let delay = *delay;
                    if let Some(ScriptItem::Reply(_, bytes)) = self.script.pop_front() {
                        if delay.is_zero() {
                            self.incoming.extend(bytes);
                        } else {
                            let target = tokio::time::Instant::now() + delay;
                            self.deliver_at = Some(target);
                            self.pending_reply = Some(bytes);
                            self.active_sleep = Some(Box::pin(tokio::time::sleep_until(target)));
                        }
                    }
                }
                _ => break,
            }
        }
    }
}

impl AsyncRead for AsyncScriptedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if let Some(err) = self.read_error {
            return Poll::Ready(Err(io::Error::from(err)));
        }

        // Check if there is an active delayed reply
        if let Some(target) = self.deliver_at {
            let mut ready = false;
            if tokio::time::Instant::now() >= target {
                ready = true;
            } else if let Some(sleep) = self.active_sleep.as_mut() {
                if sleep.as_mut().poll(cx).is_ready() {
                    ready = true;
                }
            }
            if ready {
                if let Some(bytes) = self.pending_reply.take() {
                    self.incoming.extend(bytes);
                }
                self.deliver_at = None;
                self.active_sleep = None;
                self.advance_script();
            }
        }

        if !self.incoming.is_empty() {
            let remaining = buf.remaining();
            let to_read = remaining.min(self.chunk_size).min(self.incoming.len());
            for _ in 0..to_read {
                if let Some(byte) = self.incoming.pop_front() {
                    buf.put_slice(&[byte]);
                }
            }
            return Poll::Ready(Ok(()));
        }

        if self.closed_eof {
            return Poll::Ready(Ok(()));
        }

        if let Some(sleep) = self.active_sleep.as_mut() {
            let _ = sleep.as_mut().poll(cx);
        }

        self.read_waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl AsyncWrite for AsyncScriptedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Some(err) = self.write_error {
            return Poll::Ready(Err(io::Error::from(err)));
        }
        self.written.extend_from_slice(buf);
        if let Some(ScriptItem::Write(expected)) = self.script.front().cloned() {
            if buf == expected.as_slice() {
                self.script.pop_front();
                self.advance_script();
                if let Some(waker) = self.read_waker.take() {
                    waker.wake();
                }
            } else {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("mismatched write: expected {expected:02X?}, got {buf:02X?}"),
                )));
            }
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.flush_count += 1;
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

fn link(stream: AsyncScriptedStream) -> AsyncLink<AsyncScriptedStream> {
    AsyncLink::new(stream, LinkConfig::default())
}

// -----------------------------------------------------------------------------
// Translation of tests from tests/link.rs (cases 1-17)
// -----------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn poll_writes_exact_frame_and_accepts_ack() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&[0x02, 0x30, 0x31, 0x30, 0x30, 0x03, 0x02])
        .then_reply(&[ACK]);
    let mut l = link(stream);
    l.send_command("0100").await.unwrap();
    assert!(l.stream_mut().is_finished());
}

#[tokio::test(start_paused = true)]
async fn no_ack_retries_three_times() {
    let poll = frame("0100");
    let stream = AsyncScriptedStream::new()
        .expect_write(&poll)
        .expect_write(&poll)
        .expect_write(&poll);
    let mut l = link(stream);
    let err = l.send_command("0100").await.unwrap_err();
    assert!(matches!(
        err,
        Error::NoAck {
            attempts: 3,
            naks: 0
        }
    ));
    assert_eq!(l.stream_mut().written(), poll.repeat(3));
}

#[tokio::test(start_paused = true)]
async fn nak_nak_ack_succeeds_on_third_attempt() {
    let poll = frame("0100");
    let stream = AsyncScriptedStream::new()
        .expect_write(&poll)
        .then_reply(&[NAK])
        .expect_write(&poll)
        .then_reply(&[NAK])
        .expect_write(&poll)
        .then_reply(&[ACK]);
    let mut l = link(stream);
    l.send_command("0100").await.unwrap();
    assert!(l.stream_mut().is_finished());
}

#[tokio::test(start_paused = true)]
async fn three_naks_fail_with_nak_count() {
    let poll = frame("0100");
    let stream = AsyncScriptedStream::new()
        .expect_write(&poll)
        .then_reply(&[NAK])
        .expect_write(&poll)
        .then_reply(&[NAK])
        .expect_write(&poll)
        .then_reply(&[NAK]);
    let mut l = link(stream);
    let err = l.send_command("0100").await.unwrap_err();
    assert!(matches!(
        err,
        Error::NoAck {
            attempts: 3,
            naks: 3
        }
    ));
}

#[tokio::test(start_paused = true)]
async fn sale_with_intermediate_messages() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&[ACK])
        .then_reply(&frame("0900|78"))
        .expect_write(&[ACK])
        .then_reply(&frame("0900|81"))
        .expect_write(&[ACK])
        .then_reply(&frame(SALE_RESPONSE))
        .expect_write(&[ACK]);
    let mut l = link(stream);
    let mut statuses = Vec::new();
    let mut on_status = |s| statuses.push(s);
    let response = l
        .request(SALE_REQUEST, "0210", &mut on_status)
        .await
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
    assert_eq!(l.stream_mut().written(), expected_written);
    assert!(l.stream_mut().is_finished());
}

#[tokio::test(start_paused = true)]
async fn bad_lrc_is_nakked_then_good_frame_accepted() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame("0800"))
        .then_reply(&[ACK])
        .then_reply(&corrupted("0810|00|597029414300|IT750050"))
        .expect_write(&[NAK])
        .then_reply(&frame("0810|00|597029414300|IT750050"))
        .expect_write(&[ACK]);
    let mut l = link(stream);
    let mut ignore = |_| {};
    let response = l.request("0800", "0810", &mut ignore).await.unwrap();
    assert_eq!(response, "0810|00|597029414300|IT750050");
    assert!(l.stream_mut().is_finished());
}

#[tokio::test(start_paused = true)]
async fn more_than_three_bad_lrc_fail() {
    let bad = corrupted("0810|00");
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame("0800"))
        .then_reply(&[ACK])
        .then_reply(&bad)
        .expect_write(&[NAK])
        .then_reply(&bad)
        .expect_write(&[NAK])
        .then_reply(&bad)
        .expect_write(&[NAK])
        .then_reply(&bad);
    let mut l = link(stream);
    let mut ignore = |_| {};
    let err = l.request("0800", "0810", &mut ignore).await.unwrap_err();
    assert!(matches!(err, Error::BadLrc { count: 4 }));
    assert!(l.stream_mut().is_finished());
}

#[tokio::test(start_paused = true)]
async fn garbage_is_ignored() {
    let mut reply = b"\xFF\x00zz".to_vec();
    reply.push(ACK);
    reply.extend(b"junk\x03");
    reply.extend(frame("0710|00|002|15000||"));
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame("0700||"))
        .then_reply(&reply)
        .expect_write(&[ACK]);
    let mut l = link(stream);
    let mut ignore = |_| {};
    let response = l.request("0700||", "0710", &mut ignore).await.unwrap();
    assert_eq!(response, "0710|00|002|15000||");
}

#[tokio::test(start_paused = true)]
async fn unexpected_function_code_is_an_error_after_ack() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&[ACK])
        .then_reply(&frame("0810|00"))
        .expect_write(&[ACK]);
    let mut l = link(stream);
    let mut ignore = |_| {};
    let err = l
        .request(SALE_REQUEST, "0210", &mut ignore)
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::UnexpectedResponse { expected: "0210", ref received, ref payload }
            if received == "0810" && payload == "0810|00"),
        "{err:?}"
    );
    assert!(l.stream_mut().is_finished());
}

#[tokio::test(start_paused = true)]
async fn acknowledged_sale_without_response_is_not_resent() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&[ACK]);
    let mut l = link(stream);
    let mut ignore = |_| {};
    let err = l
        .request(SALE_REQUEST, "0210", &mut ignore)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::ResponseTimeout { waited } if waited == Duration::from_secs(150)));
    let written = l.stream_mut().written().to_vec();
    assert_eq!(count_occurrences(&written, &frame(SALE_REQUEST)), 1);
    assert_eq!(written, frame(SALE_REQUEST));
}

#[tokio::test(start_paused = true)]
async fn response_without_ack_is_an_implicit_ack() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&frame(SALE_RESPONSE))
        .expect_write(&[ACK]);
    let mut l = link(stream);
    let mut ignore = |_| {};
    let response = l.request(SALE_REQUEST, "0210", &mut ignore).await.unwrap();
    assert_eq!(response, SALE_RESPONSE);
    assert_eq!(
        count_occurrences(l.stream_mut().written(), &frame(SALE_REQUEST)),
        1
    );
}

#[tokio::test(start_paused = true)]
async fn intermediate_messages_do_not_extend_the_deadline() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&[ACK])
        .then_reply_after(Duration::from_secs(100), &frame("0900|78"))
        .expect_write(&[ACK])
        .then_reply_after(Duration::from_secs(100), &frame("0900|81"));
    let mut l = link(stream);
    let mut statuses = 0;
    let mut on_status = |_| statuses += 1;
    let result = l.request(SALE_REQUEST, "0210", &mut on_status).await;
    assert!(matches!(result, Err(Error::ResponseTimeout { .. })));
    assert_eq!(statuses, 1);
}

fn detail(n: u32) -> String {
    format!(
        "0261|00|597029414300|IT750050|{n:06}|{:06}|1000|9480|{n:06}|CR|22102025|0000000000000000000|MC|22102025|114016|1|0|0|0",
        700_000 + n
    )
}

const END: &str = "0261|00|597029414300|IT750050||||||||||||||||";

fn list_script(payloads: &[String]) -> AsyncScriptedStream {
    let mut stream = AsyncScriptedStream::new()
        .expect_write(&frame("0260|1|"))
        .then_reply(&[ACK]);
    for p in payloads {
        stream = stream.then_reply(&frame(p)).expect_write(&[ACK]);
    }
    stream
}

#[tokio::test(start_paused = true)]
async fn detail_list_with_two_terminators() {
    let mut payloads: Vec<String> = (1..=3).map(detail).collect();
    payloads.push(END.to_owned());
    payloads.push(END.to_owned());
    let mut l = link(list_script(&payloads));
    let entries = l.request_list("0260|1|", "0261").await.unwrap();
    assert_eq!(entries, [detail(1), detail(2), detail(3)]);
    assert!(l.stream_mut().is_finished());
}

#[tokio::test(start_paused = true)]
async fn detail_list_with_one_terminator_waits_the_tail() {
    let mut payloads: Vec<String> = (1..=3).map(detail).collect();
    payloads.push(END.to_owned());
    let mut l = link(list_script(&payloads));
    let entries = l.request_list("0260|1|", "0261").await.unwrap();
    assert_eq!(entries.len(), 3);
}

#[tokio::test(start_paused = true)]
async fn detail_list_without_sales() {
    let payloads = ["0261|11||||||||||||||||||".to_owned()];
    let mut l = link(list_script(&payloads));
    assert_eq!(
        l.request_list("0260|1|", "0261").await.unwrap(),
        Vec::<String>::new()
    );
}

#[tokio::test(start_paused = true)]
async fn detail_list_is_bounded() {
    let payloads: Vec<String> = (1..=10_001).map(detail).collect();
    let mut l = link(list_script(&payloads));
    assert!(matches!(
        l.request_list("0260|1|", "0261").await,
        Err(Error::TooManyFrames { max: 10_000 })
    ));
}

#[test]
fn link_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<AsyncLink<AsyncScriptedStream>>();
    assert_send::<AsyncLink<tokio::net::TcpStream>>();
}

// -----------------------------------------------------------------------------
// Translation of tests from tests/link_edge_cases.rs (cases 18-31)
// -----------------------------------------------------------------------------

fn sale_script(response: &str) -> AsyncScriptedStream {
    AsyncScriptedStream::new()
        .expect_write(&frame("0250|"))
        .then_reply(&[ACK])
        .then_reply(&frame(response))
        .expect_write(&[ACK])
}

#[tokio::test(start_paused = true)]
async fn byte_by_byte_delivery() {
    let response = "0260|00|597029414300|IT750050|ABC123|757752|12000";
    let mut stream = sale_script(response);
    stream.set_chunk_size(1);
    let mut l = link(stream);
    let mut ignore = |_| {};
    let res = l.request("0250|", "0260", &mut ignore).await.unwrap();
    assert_eq!(res, response);
}

fn payload_with_lrc(target: u8) -> String {
    (0..1_000)
        .find_map(|n| {
            let prefix = format!("0260|00|{n}");
            let c = lrc(prefix.as_bytes()) ^ target;
            ((0x20..0x7F).contains(&c) && c != b'|').then(|| format!("{prefix}{}", char::from(c)))
        })
        .unwrap_or_default()
}

#[tokio::test(start_paused = true)]
async fn responses_whose_lrc_is_a_control_byte() {
    for target in [0x02, 0x03, ACK, NAK] {
        let payload = payload_with_lrc(target);
        assert_eq!(frame(&payload).last(), Some(&target));
        let mut l = link(sale_script(&payload));
        let mut ignore = |_| {};
        assert_eq!(
            l.request("0250|", "0260", &mut ignore).await.unwrap(),
            payload
        );
        assert!(l.stream_mut().is_finished());
    }
}

#[tokio::test(start_paused = true)]
async fn stale_input_is_discarded_before_sending() {
    let mut stream = sale_script("0260|00|1");
    // Stale response pushed into incoming before sending command
    stream.push_incoming(&frame("0260|00|OLD"));
    stream.push_incoming(&[ACK]);
    let mut l = link(stream);
    let mut ignore = |_| {};
    assert_eq!(
        l.request("0250|", "0260", &mut ignore).await.unwrap(),
        "0260|00|1"
    );
}

#[tokio::test(start_paused = true)]
async fn io_errors_while_waiting_propagate() {
    for kind in [io::ErrorKind::UnexpectedEof, io::ErrorKind::BrokenPipe] {
        let mut stream = AsyncScriptedStream::new();
        stream.set_read_error(kind);
        let mut l = link(stream);
        let err = l.send_command("0100").await.unwrap_err();
        assert!(
            matches!(err, Error::Io(ref e) if e.kind() == kind),
            "{err:?}"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn write_errors_propagate() {
    let mut stream = AsyncScriptedStream::new();
    stream.set_write_error(io::ErrorKind::BrokenPipe);
    let mut l = link(stream);
    assert!(matches!(l.send_command("0100").await, Err(Error::Io(_))));
}

#[tokio::test(start_paused = true)]
async fn invalid_payloads_are_never_written() {
    let stream = AsyncScriptedStream::new();
    let mut l = link(stream);
    assert!(matches!(
        l.send_command("").await,
        Err(Error::InvalidArgument { .. })
    ));
    let mut ignore = |_| {};
    assert!(matches!(
        l.request("02\u{3}00", "0210", &mut ignore).await,
        Err(Error::InvalidArgument { .. })
    ));
    assert!(matches!(
        l.request_list(&"1".repeat(5000), "0261").await,
        Err(Error::Overflow { .. })
    ));
    assert_eq!(l.stream_mut().written(), b"");
}

#[tokio::test(start_paused = true)]
async fn zero_attempts_still_sends_once() {
    let mut config = LinkConfig::default();
    config.max_attempts = 0;
    let stream = AsyncScriptedStream::new().expect_write(&frame("0100"));
    let mut l = AsyncLink::new(stream, config);
    assert!(matches!(
        l.send_command("0100").await,
        Err(Error::NoAck {
            attempts: 1,
            naks: 0
        })
    ));
}

#[tokio::test(start_paused = true)]
async fn custom_timeouts_are_respected() {
    let mut config = LinkConfig::default();
    config.ack_timeout = Duration::from_millis(300);
    config.max_attempts = 5;
    config.response_timeout = Duration::from_secs(7);

    let poll = frame("0100");
    let mut stream = AsyncScriptedStream::new();
    for _ in 0..5 {
        stream = stream.expect_write(&poll);
    }
    let mut l = AsyncLink::new(stream, config.clone());
    assert!(matches!(
        l.send_command("0100").await,
        Err(Error::NoAck { attempts: 5, .. })
    ));
    assert_eq!(l.config(), &config);

    let stream = AsyncScriptedStream::new()
        .expect_write(&frame("0700||"))
        .then_reply(&[ACK]);
    let mut l = AsyncLink::new(stream, config);
    let mut ignore = |_| {};
    assert!(matches!(
        l.request("0700||", "0710", &mut ignore).await,
        Err(Error::ResponseTimeout { waited }) if waited == Duration::from_secs(7)
    ));
}

#[tokio::test(start_paused = true)]
async fn bad_lrc_limit_is_configurable() {
    let mut config = LinkConfig::default();
    config.max_bad_lrc = 0;
    let mut bad = frame("0710|00");
    if let Some(last) = bad.last_mut() {
        *last ^= 0xFF;
    }
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame("0700||"))
        .then_reply(&[ACK])
        .then_reply(&bad);
    let mut l = AsyncLink::new(stream, config);
    let mut ignore = |_| {};
    assert!(matches!(
        l.request("0700||", "0710", &mut ignore).await,
        Err(Error::BadLrc { count: 1 })
    ));
}

#[tokio::test(start_paused = true)]
async fn bad_lrc_counter_resets_after_a_good_frame() {
    let mut bad = frame("0900|78");
    if let Some(last) = bad.last_mut() {
        *last ^= 0xFF;
    }
    let mut stream = AsyncScriptedStream::new()
        .expect_write(&frame("0200|000001000|1||0|1"))
        .then_reply(&[ACK]);
    // 3 bad + 1 good intermediate, twice: never 4 bad in a row.
    for _ in 0..2 {
        for _ in 0..3 {
            stream = stream.then_reply(&bad).expect_write(&[NAK]);
        }
        stream = stream.then_reply(&frame("0900|78")).expect_write(&[ACK]);
    }
    stream = stream.then_reply(&frame("0210|00|1")).expect_write(&[ACK]);
    let mut l = link(stream);
    let mut statuses = 0;
    let mut on_status = |_| statuses += 1;
    let response = l
        .request("0200|000001000|1||0|1", "0210", &mut on_status)
        .await
        .unwrap();
    assert_eq!(response, "0210|00|1");
    assert_eq!(statuses, 2);
}

#[tokio::test(start_paused = true)]
async fn list_with_intermediate_garbage_and_bad_lrc_in_the_tail() {
    let entry = "0261|00|1|T|1|123456|1000";
    let end = "0261|00|1|T||";
    let mut bad_end = frame(end);
    if let Some(last) = bad_end.last_mut() {
        *last ^= 0xFF;
    }
    let mut garbage_then_entry = b"\x00junk".to_vec();
    garbage_then_entry.extend(frame(entry));
    let stream = AsyncScriptedStream::new()
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
    let mut l = link(stream);
    assert_eq!(l.request_list("0260|1|", "0261").await.unwrap(), [entry]);
    assert!(l.stream_mut().is_finished());
}

#[tokio::test(start_paused = true)]
async fn wrong_frame_in_list_tail_is_unexpected() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame("0260|1|"))
        .then_reply(&[ACK])
        .then_reply(&frame("0261|11||||"))
        .expect_write(&[ACK])
        .then_reply(&frame("0810|00"))
        .expect_write(&[ACK]);
    let mut l = link(stream);
    assert!(matches!(
        l.request_list("0260|1|", "0261").await,
        Err(Error::UnexpectedResponse {
            expected: "0261",
            ..
        })
    ));
}

#[tokio::test(start_paused = true)]
async fn list_entry_after_the_first_terminator_is_kept() {
    let late = "0261|00|1|T|2|654321|2000";
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame("0260|1|"))
        .then_reply(&[ACK])
        .then_reply(&frame("0261|00|1|T||"))
        .expect_write(&[ACK])
        .then_reply_after(Duration::from_millis(400), &frame(late))
        .expect_write(&[ACK]);
    let mut l = link(stream);
    assert_eq!(l.request_list("0260|1|", "0261").await.unwrap(), [late]);
}

#[tokio::test(start_paused = true)]
async fn into_inner_returns_the_stream() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame("0100"))
        .then_reply(&[ACK]);
    let mut l = link(stream);
    l.send_command("0100").await.unwrap();
    assert_eq!(l.into_inner().written(), frame("0100"));
}

// -----------------------------------------------------------------------------
// Mandatory traps a–f tests
// -----------------------------------------------------------------------------

/// Trap a: in `AsyncRead`, reading 0 bytes is a closed connection, returning `UnexpectedEof`.
#[tokio::test(start_paused = true)]
async fn trap_a_eof_returns_unexpected_eof() {
    let mut stream = AsyncScriptedStream::new().expect_write(&frame("0100"));
    stream.close();
    let mut l = link(stream);
    let err = l.send_command("0100").await.unwrap_err();
    assert!(
        matches!(err, Error::Io(ref e) if e.kind() == io::ErrorKind::UnexpectedEof),
        "{err:?}"
    );
}

/// Trap b: stale unread input is drained before sending a new command without blocking.
#[tokio::test(start_paused = true)]
async fn trap_b_stale_input_drained_before_command() {
    let mut stream = AsyncScriptedStream::new()
        .expect_write(&frame("0100"))
        .then_reply(&[ACK]);
    // Stale bytes pushed before the operation:
    stream.push_incoming(b"stale trash 0210|00");
    let mut l = link(stream);
    l.send_command("0100").await.unwrap();
    assert!(l.stream_mut().is_finished());
}

/// Trap c: flush is called after each `write_all`, including ACK and NAK.
#[tokio::test(start_paused = true)]
async fn trap_c_flush_called_after_all_writes() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame("0800"))
        .then_reply(&[ACK])
        .then_reply(&corrupted("0810|00|SIM00001"))
        .expect_write(&[NAK])
        .then_reply(&frame("0810|00|SIM00001"))
        .expect_write(&[ACK]);
    let mut l = link(stream);
    let mut ignore = |_| {};
    l.request("0800", "0810", &mut ignore).await.unwrap();
    // Writes: frame(0800), NAK, ACK = 3 writes total, flush called 3 times.
    assert_eq!(l.stream_mut().flush_count(), 3);
}

/// Trap d: deadlines use `timeout_at` and are never extended by 0900 messages.
#[tokio::test(start_paused = true)]
async fn trap_d_plazos_never_extended_by_0900() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&[ACK])
        .then_reply_after(Duration::from_secs(149), &frame("0900|78"))
        .expect_write(&[ACK]);
    let mut l = link(stream);
    let mut count = 0;
    let mut on_status = |_| count += 1;
    let res = l.request(SALE_REQUEST, "0210", &mut on_status).await;
    assert!(
        matches!(res, Err(Error::ResponseTimeout { waited }) if waited == Duration::from_secs(150))
    );
    assert_eq!(count, 1);
}

/// Trap e: sale is never resent after ACK received (LINK-03).
#[tokio::test(start_paused = true)]
async fn trap_e_sale_never_resent_after_ack() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&[ACK]);
    let mut l = link(stream);
    let mut ignore = |_| {};
    let err = l
        .request(SALE_REQUEST, "0210", &mut ignore)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::ResponseTimeout { .. }));
    let written = l.stream_mut().written();
    assert_eq!(count_occurrences(written, &frame(SALE_REQUEST)), 1);
}

/// Trap f: external timeout dropping sale leaves result unknown; next operation works cleanly.
#[tokio::test(start_paused = true)]
async fn trap_f_external_cancellation_leaves_link_usable() {
    let stream = AsyncScriptedStream::new()
        .expect_write(&frame(SALE_REQUEST))
        .then_reply(&[ACK]);
    let mut l = link(stream);
    let mut ignore = |_| {};
    let sale_fut = l.request(SALE_REQUEST, "0210", &mut ignore);
    // External cancellation dropping the future:
    let timeout_res = tokio::time::timeout(Duration::from_millis(50), sale_fut).await;
    assert!(timeout_res.is_err());

    // Next operation on the same link: poll
    l.stream_mut()
        .script
        .push_back(ScriptItem::Write(frame("0100")));
    l.stream_mut()
        .script
        .push_back(ScriptItem::Reply(Duration::ZERO, vec![ACK]));
    l.send_command("0100").await.unwrap();
    assert!(l.stream_mut().is_finished());
}
