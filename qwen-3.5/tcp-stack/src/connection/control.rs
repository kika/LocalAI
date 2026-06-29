//! TCP Control Block (PCB)

use super::TcpState;
use crate::congestion::NewReno;
use crate::flow_control::SlidingWindow;
use crate::reliability::{ReorderBuffer, RetransmissionManager};
use crate::utils::SeqNumber;
use std::time::Instant;

/// Protocol Control Block
pub struct ControlBlock {
  pub state: TcpState,

  pub send_seq: SeqNumber,
  pub send_una: SeqNumber,
  pub send_nxt: SeqNumber,
  pub send_wnd: u32,

  pub recv_seq: SeqNumber,
  pub recv_ack: SeqNumber,
  pub recv_wnd: u32,

  pub congestion: NewReno,
  pub send_window: SlidingWindow,
  pub recv_buffer: ReorderBuffer,
  pub retransmit: RetransmissionManager,

  pub rtt_estimator: RttEstimator,
  pub mss: u16,
  pub window_scale: u8,

  pub last_activity: Instant,
}

impl ControlBlock {
  pub fn new() -> Self {
    let initial_seq = SeqNumber::random();

    Self {
      state: TcpState::Closed,
      send_seq: initial_seq,
      send_una: initial_seq,
      send_nxt: initial_seq,
      send_wnd: 65535,

      recv_seq: SeqNumber(0),
      recv_ack: SeqNumber(0),
      recv_wnd: 65535,

      congestion: NewReno::new(),
      send_window: SlidingWindow::new(65535),
      recv_buffer: ReorderBuffer::new(),
      retransmit: RetransmissionManager::new(),

      rtt_estimator: RttEstimator::new(),
      mss: 1460,
      window_scale: 7,

      last_activity: Instant::now(),
    }
  }

  pub fn update_activity(&mut self) {
    self.last_activity = Instant::now();
  }
}

impl Default for ControlBlock {
  fn default() -> Self {
    Self::new()
  }
}

/// RTT Estimator using Jacobson's algorithm
pub struct RttEstimator {
  srtt: f64,
  rttvar: f64,
  rto: f64,
}

impl RttEstimator {
  pub fn new() -> Self {
    Self {
      srtt: 0.0,
      rttvar: 0.0,
      rto: 1.0,
    }
  }

  pub fn update(&mut self, rtt: f64) {
    if self.srtt == 0.0 {
      self.srtt = rtt;
      self.rttvar = rtt / 2.0;
    } else {
      let alpha = 0.125;
      let beta = 0.25;

      let diff = (rtt - self.srtt).abs();
      self.rttvar = (1.0 - beta) * self.rttvar + beta * diff;
      self.srtt = (1.0 - alpha) * self.srtt + alpha * rtt;
    }

    self.rto = (self.srtt + 4.0 * self.rttvar).max(1.0);
  }

  pub fn rto(&self) -> f64 {
    self.rto
  }

  pub fn srtt(&self) -> f64 {
    self.srtt
  }
}

impl Default for RttEstimator {
  fn default() -> Self {
    Self::new()
  }
}
