//! NewReno congestion control algorithm

use crate::utils::SeqNumber;

/// NewReno congestion control state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CongestionState {
  SlowStart,
  CongestionAvoidance,
  FastRecovery,
}

/// NewReno congestion control
pub struct NewReno {
  cwnd: u32,
  ssthresh: u32,
  state: CongestionState,
  dup_acks: u32,
  last_cwnd_reduction: SeqNumber,
  initial_mss: u32,
}

impl NewReno {
  pub fn new() -> Self {
    let initial_mss = 1460;
    Self {
      cwnd: initial_mss,
      ssthresh: u32::MAX,
      state: CongestionState::SlowStart,
      dup_acks: 0,
      last_cwnd_reduction: SeqNumber(0),
      initial_mss,
    }
  }

  pub fn on_ack(&mut self, ack: SeqNumber, bytes_acked: u32) {
    match self.state {
      CongestionState::SlowStart => {
        self.cwnd += bytes_acked;
        if self.cwnd >= self.ssthresh {
          self.state = CongestionState::CongestionAvoidance;
          self.cwnd = self.ssthresh + 2 * self.initial_mss;
        }
      }
      CongestionState::CongestionAvoidance => {
        self.cwnd += self.initial_mss * bytes_acked / self.cwnd;
      }
      CongestionState::FastRecovery => {
        if ack.after(self.last_cwnd_reduction) {
          self.state = CongestionState::CongestionAvoidance;
          self.cwnd = self.ssthresh;
        }
      }
    }

    self.dup_acks = 0;
  }

  pub fn on_duplicate_ack(&mut self) {
    self.dup_acks += 1;

    if self.dup_acks == 3 {
      self.enter_fast_retransmit();
    } else if self.dup_acks > 3 && self.state == CongestionState::FastRecovery {
      self.cwnd += self.initial_mss;
    }
  }

  fn enter_fast_retransmit(&mut self) {
    self.ssthresh = (self.cwnd / 2).max(2 * self.initial_mss);
    self.cwnd = self.ssthresh + 3 * self.initial_mss;
    self.state = CongestionState::FastRecovery;
    self.dup_acks = 3;
  }

  pub fn on_timeout(&mut self) {
    self.ssthresh = (self.cwnd / 2).max(2 * self.initial_mss);
    self.cwnd = self.initial_mss;
    self.state = CongestionState::SlowStart;
    self.dup_acks = 0;
  }

  pub fn cwnd(&self) -> u32 {
    self.cwnd
  }

  pub fn ssthresh(&self) -> u32 {
    self.ssthresh
  }

  pub fn state(&self) -> CongestionState {
    self.state
  }
}

impl Default for NewReno {
  fn default() -> Self {
    Self::new()
  }
}
