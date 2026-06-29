//! Integration tests for TCP stack

use std::net::Ipv4Addr;
use tcp_stack::packet::{Ipv4Header, TcpFlags, TcpHeader, TcpOption};
use tcp_stack::utils::{SeqNumber, calculate_checksum};

#[test]
fn test_ipv4_header_serialization() {
  let src = Ipv4Addr::new(192, 168, 1, 1);
  let dst = Ipv4Addr::new(192, 168, 1, 2);

  let header = Ipv4Header::new(src, dst, 100);
  let bytes = header.serialize();

  assert_eq!(bytes.len(), 20);
  assert_eq!(bytes[0] >> 4, 4); // Version
  assert_eq!(bytes[9], 6); // Protocol (TCP)
}

#[test]
fn test_tcp_header_syn() {
  let header = TcpHeader::syn(12345, 80, 1000, 1460);

  assert!(header.flags.is_syn());
  assert!(!header.flags.is_ack());
  assert_eq!(header.seq_num, 1000);
  assert_eq!(header.src_port, 12345);
  assert_eq!(header.dst_port, 80);

  let bytes = header.serialize();
  assert!(bytes.len() >= 20);
}

#[test]
fn test_tcp_header_syn_ack() {
  let header = TcpHeader::syn_ack(80, 12345, 2000, 1001, 1460);

  assert!(header.flags.is_syn());
  assert!(header.flags.is_ack());
  assert_eq!(header.seq_num, 2000);
  assert_eq!(header.ack_num, 1001);
}

#[test]
fn test_tcp_header_ack() {
  let mut header = TcpHeader::new(12345, 80);
  header.flags = TcpFlags::new().with_ack();
  header.seq_num = 1000;
  header.ack_num = 2000;

  assert!(header.flags.is_ack());
  assert!(!header.flags.is_syn());
}

#[test]
fn test_sequence_number_arithmetic() {
  let seq1 = SeqNumber(100);
  let seq2 = SeqNumber(200);

  assert!(seq1.before(seq2));
  assert!(seq2.after(seq1));

  let seq3 = seq1 + 50;
  assert_eq!(seq3.0, 150);

  let seq4 = SeqNumber(u32::MAX);
  let seq5 = seq4 + 1;
  assert_eq!(seq5.0, 0); // Wrap around
}

#[test]
fn test_checksum_basic() {
  let data = vec![0x00, 0x01, 0x02, 0x03];
  let checksum = calculate_checksum(&data);
  assert_ne!(checksum, 0);
}

#[test]
fn test_pseudo_header_checksum() {
  let src = u32::from_be_bytes([192, 168, 1, 1]);
  let dst = u32::from_be_bytes([192, 168, 1, 2]);

  let checksum = tcp_stack::utils::calculate_pseudo_header_checksum(src, dst, 6, 20);
  assert_ne!(checksum, 0);
}

#[test]
fn test_tcp_options_mss() {
  let option = TcpOption::MaximumSegmentSize(1460);
  let bytes = option.serialize();

  assert_eq!(bytes[0], 2); // Kind
  assert_eq!(bytes[1], 4); // Length
  assert_eq!(u16::from_be_bytes([bytes[2], bytes[3]]), 1460);
}

#[test]
fn test_tcp_options_window_scale() {
  let option = TcpOption::WindowScale(7);
  let bytes = option.serialize();

  assert_eq!(bytes[0], 3); // Kind
  assert_eq!(bytes[1], 3); // Length
  assert_eq!(bytes[2], 7); // Scale
}

#[test]
fn test_tcp_options_sack_permitted() {
  let option = TcpOption::SackPermitted;
  let bytes = option.serialize();

  assert_eq!(bytes[0], 4); // Kind
  assert_eq!(bytes[1], 2); // Length
}

#[test]
fn test_tcp_options_timestamp() {
  let option = TcpOption::Timestamp {
    ts_val: 1000,
    ts_ecr: 500,
  };
  let bytes = option.serialize();

  assert_eq!(bytes[0], 8); // Kind
  assert_eq!(bytes[1], 10); // Length
  assert_eq!(
    u32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]),
    1000
  );
  assert_eq!(
    u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
    500
  );
}

#[test]
fn test_connection_state_transitions() {
  use tcp_stack::connection::TcpState;

  let mut state = TcpState::Closed;
  assert!(state.is_closed());

  state = TcpState::Listen;
  assert!(!state.is_closed());

  state = TcpState::SynSent;
  assert!(state.is_syn_sent());

  state = TcpState::Established;
  assert!(state.is_established());
}

#[test]
fn test_sliding_window() {
  use tcp_stack::flow_control::SlidingWindow;

  let mut window = SlidingWindow::new(1000);
  let seq = SeqNumber(0);

  assert!(window.can_send(seq, 500));
  assert!(window.can_send(seq, 1000));
  assert!(!window.can_send(seq, 1001));

  window.advance(SeqNumber(500));
  assert!(window.can_send(SeqNumber(500), 500));
}

#[test]
fn test_reorder_buffer() {
  use tcp_stack::reliability::ReorderBuffer;

  let mut buffer = ReorderBuffer::new();

  // Add out-of-order segment
  let ready = buffer.add(SeqNumber(100), vec![1, 2, 3]);
  assert!(ready.is_empty());

  // Add expected segment
  let ready = buffer.add(SeqNumber(0), vec![4, 5, 6]);
  assert_eq!(ready.len(), 1);
  assert_eq!(ready[0].0, SeqNumber(0));
}

#[test]
fn test_newreno_congestion_control() {
  use tcp_stack::congestion::NewReno;

  let mut cc = NewReno::new();
  let initial_cwnd = cc.cwnd();

  // Slow start
  cc.on_ack(SeqNumber(100), 1460);
  assert!(cc.cwnd() > initial_cwnd);

  // Simulate packet loss
  cc.on_timeout();
  assert_eq!(cc.cwnd(), 1460); // Back to 1 MSS
}
