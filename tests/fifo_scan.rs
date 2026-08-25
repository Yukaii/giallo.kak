//! Tests for the FIFO reader's sentinel scanning logic.

use giallo_kak::fifo::drain_messages;

const SENTINEL: &str = "giallo-1a2b3c4d";

#[test]
fn extracts_single_message() {
    let mut buf = format!("hello world{SENTINEL}");
    let mut scanned = 0;
    let msgs = drain_messages(&mut buf, SENTINEL, &mut scanned);
    assert_eq!(msgs, vec!["hello world"]);
    assert!(buf.is_empty());
}

#[test]
fn extracts_multiple_messages_in_one_pass() {
    let mut buf = format!("first{SENTINEL}second{SENTINEL}leftover");
    let mut scanned = 0;
    let msgs = drain_messages(&mut buf, SENTINEL, &mut scanned);
    assert_eq!(msgs, vec!["first", "second"]);
    assert_eq!(buf, "leftover");
}

#[test]
fn no_match_leaves_buffer_intact() {
    let mut buf = String::from("partial data without sentinel");
    let mut scanned = 0;
    let msgs = drain_messages(&mut buf, SENTINEL, &mut scanned);
    assert!(msgs.is_empty());
    assert_eq!(buf, "partial data without sentinel");
    // Scan position must be rewound so a boundary-spanning match is possible.
    assert!(scanned + SENTINEL.len() - 1 >= buf.len());
}

#[test]
fn message_split_across_appends_is_found() {
    let mut buf = String::new();
    let mut scanned = 0;

    // Simulate chunked FIFO reads splitting the sentinel in half.
    let (a, b) = SENTINEL.split_at(SENTINEL.len() / 2);
    buf.push_str("payload-");
    buf.push_str(a);
    assert!(drain_messages(&mut buf, SENTINEL, &mut scanned).is_empty());

    buf.push_str(b);
    buf.push_str("-tail");
    let msgs = drain_messages(&mut buf, SENTINEL, &mut scanned);
    assert_eq!(msgs, vec!["payload-"]);
    assert_eq!(buf, "-tail");
}

#[test]
fn rescan_skips_already_scanned_prefix() {
    let mut buf = String::new();
    let mut scanned = 0;

    // Large body with no sentinel: after an unsuccessful scan the resume
    // offset must cover everything except a potential sentinel suffix.
    buf.push_str(&"x".repeat(10_000));
    drain_messages(&mut buf, SENTINEL, &mut scanned);
    let scanned_after_first = scanned;
    assert!(scanned_after_first >= 10_000 - (SENTINEL.len() - 1));

    // Append more sentinel-free data: only the tail region gets rescanned,
    // and nothing is extracted.
    buf.push_str(&"y".repeat(5_000));
    let msgs = drain_messages(&mut buf, SENTINEL, &mut scanned);
    assert!(msgs.is_empty());
    assert!(scanned > scanned_after_first);
    assert_eq!(buf.len(), 15_000);
}

#[test]
fn complete_message_after_large_accumulation() {
    let mut buf = String::new();
    let mut scanned = 0;

    buf.push_str(&"x".repeat(50_000));
    assert!(drain_messages(&mut buf, SENTINEL, &mut scanned).is_empty());

    // Sentinel arrives spanning the append boundary.
    let (a, b) = SENTINEL.split_at(3);
    buf.push_str("msg");
    let expected = buf.clone();
    buf.push_str(a);
    assert!(drain_messages(&mut buf, SENTINEL, &mut scanned).is_empty());
    buf.push_str(b);
    let msgs = drain_messages(&mut buf, SENTINEL, &mut scanned);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0], expected);
}
