//! Retransmission management

use crate::connection::timer::Timer;
use crate::utils::SeqNumber;
use std::collections::HashMap;
use std::time::Duration;

/// Segment awaiting acknowledgment
#[derive(Debug, Clone)]
pub struct PendingSegment {
  pub seq: SeqNumber,
  pub len: u32,
  pub data: Vec<u8>,
  pub retransmit_count: u32,
  pub first_sent: std::time::Instant,
}

/// Retransmission manager
pub struct RetransmissionManager {
  pending: HashMap<u32, PendingSegment>,
  timer: Timer,
  max_retries: u32,
}

impl RetransmissionManager {
  pub fn new() -> Self {
    Self {
      pending: HashMap::new(),
      timer: Timer::new(),
      max_retries: 15,
    }
  }

  pub fn add_segment(&mut self, segment: PendingSegment, rto: f64) {
    let key = segment.seq.0;
    self.pending.insert(key, segment);

    if self.pending.len() == 1 {
      self.timer.start(Duration::from_secs_f64(rto));
    }
  }

  pub fn acknowledge(&mut self, ack: SeqNumber) -> Vec<PendingSegment> {
    let mut acknowledged = Vec::new();

    let ack_val = ack.0;
    let keys_to_remove: Vec<u32> = self
      .pending
      .iter()
      .filter(|(_, seg)| {
        let seg_end = seg.seq.0.wrapping_add(seg.len);
        ack_val >= seg_end
      })
      .map(|(k, _)| *k)
      .collect();

    for key in keys_to_remove {
      if let Some(seg) = self.pending.remove(&key) {
        acknowledged.push(seg);
      }
    }

    if !self.pending.is_empty() {
      let min_rto = self
        .pending
        .values()
        .map(|s| {
          let elapsed = s.first_sent.elapsed().as_secs_f64();
          (1.0 + elapsed * 2.0).min(60.0)
        })
        .fold(f64::MAX, f64::min);
      self.timer.start(Duration::from_secs_f64(min_rto));
    } else {
      self.timer.cancel();
    }

    acknowledged
  }

  pub fn should_retransmit(&self) -> bool {
    self.timer.is_expired() && !self.pending.is_empty()
  }

  pub fn get_retransmit_segments(&mut self, rto: f64) -> Vec<PendingSegment> {
    if !self.should_retransmit() {
      return Vec::new();
    }

    let mut segments = Vec::new();
    for (_, seg) in self.pending.iter_mut() {
      seg.retransmit_count += 1;
      if seg.retransmit_count <= self.max_retries {
        segments.push(seg.clone());
      }
    }

    self.timer.start(Duration::from_secs_f64(rto * 2.0));
    segments
  }

  pub fn clear(&mut self) {
    self.pending.clear();
    self.timer.cancel();
  }

  pub fn pending_count(&self) -> usize {
    self.pending.len()
  }
}

impl Default for RetransmissionManager {
  fn default() -> Self {
    Self::new()
  }
}
