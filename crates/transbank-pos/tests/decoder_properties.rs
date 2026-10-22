//! Property tests of the frame codec (PROTO-04): round trip, chunking invariance
//! and absence of panics.

use proptest::collection::vec;
use proptest::prelude::*;
use transbank_pos::Result;
use transbank_pos::protocol::{ACK, FrameDecoder, LinkEvent, NAK, STX, encode_frame};

#[derive(Debug, Clone)]
enum Item {
    Frame(String),
    Ack,
    Nak,
    Garbage(Vec<u8>),
}

fn printable(max: usize) -> impl Strategy<Value = String> {
    vec(0x20u8..=0x7E, 1..=max).prop_map(|bytes| bytes.into_iter().map(char::from).collect())
}

fn item() -> impl Strategy<Value = Item> {
    prop_oneof![
        printable(60).prop_map(Item::Frame),
        Just(Item::Ack),
        Just(Item::Nak),
        vec(
            any::<u8>().prop_filter("not significant", |b| ![STX, ACK, NAK].contains(b)),
            1..10
        )
        .prop_map(Item::Garbage),
    ]
}

fn to_bytes(items: &[Item]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for item in items {
        match item {
            Item::Frame(payload) => bytes.extend(encode_frame(payload)?),
            Item::Ack => bytes.push(ACK),
            Item::Nak => bytes.push(NAK),
            Item::Garbage(garbage) => bytes.extend(garbage),
        }
    }
    Ok(bytes)
}

fn decode_chunked(bytes: &[u8], sizes: &[usize]) -> Vec<LinkEvent> {
    let mut decoder = FrameDecoder::new();
    let mut events = Vec::new();
    let mut rest = bytes;
    let mut sizes = sizes.iter().cycle();
    while !rest.is_empty() {
        let size = sizes.next().copied().unwrap_or(usize::MAX).min(rest.len());
        let (chunk, tail) = rest.split_at(size);
        decoder.push(chunk, &mut events);
        rest = tail;
    }
    events
}

fn garbage_total(events: &[LinkEvent]) -> usize {
    events
        .iter()
        .map(|event| match event {
            LinkEvent::Garbage(n) => *n,
            _ => 0,
        })
        .sum()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn round_trip(payload in printable(200)) {
        let mut decoder = FrameDecoder::new();
        let mut events = Vec::new();
        decoder.push(&encode_frame(&payload).unwrap(), &mut events);
        prop_assert_eq!(events, vec![LinkEvent::Frame(payload.into_bytes())]);
        prop_assert!(decoder.is_idle());
    }

    #[test]
    fn chunking_does_not_change_events(
        items in vec(item(), 0..20),
        sizes in vec(1usize..16, 1..8),
    ) {
        let bytes = to_bytes(&items).unwrap();
        let whole = decode_chunked(&bytes, &[usize::MAX]);
        let chunked = decode_chunked(&bytes, &sizes);
        prop_assert_eq!(garbage_total(&whole), garbage_total(&chunked));
        prop_assert_eq!(&whole, &chunked);

        let frames: Vec<Vec<u8>> = items
            .iter()
            .filter_map(|item| match item {
                Item::Frame(payload) => Some(payload.clone().into_bytes()),
                _ => None,
            })
            .collect();
        let decoded: Vec<Vec<u8>> = whole
            .into_iter()
            .filter_map(|event| match event {
                LinkEvent::Frame(payload) => Some(payload),
                _ => None,
            })
            .collect();
        prop_assert_eq!(decoded, frames);
    }

    #[test]
    fn arbitrary_bytes_never_panic(bytes in vec(any::<u8>(), 0..2000), size in 1usize..64) {
        let events = decode_chunked(&bytes, &[size]);
        prop_assert!(events.len() <= bytes.len());
    }
}
