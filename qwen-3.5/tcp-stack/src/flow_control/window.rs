//! Sliding window for flow control

use crate::utils::SeqNumber;

/// Sliding window for sender
pub struct SlidingWindow {
  size: u32,
  left_edge: SeqNumber,
  right_edge: SeqNumber,
}

impl SlidingWindow {
  pub fn new(size: u32) -> Self {
    Self {
      size,
      left_edge: SeqNumber(0),
      right_edge: SeqNumber(size),
    }
  }

  pub fn advance(&mut self, ack: SeqNumber) {
    if ack.after(self.left_edge) {
      self.left_edge = ack;
      self.right_edge = ack + self.size;
    }
  }

  pub fn can_send(&self, seq: SeqNumber, len: u32) -> bool {
    let seg_end = seq + len;
    !seg_end.after(self.right_edge)
  }

  pub fn available(&self, next_seq: SeqNumber) -> u32 {
    if next_seq.0 == self.right_edge.0 || next_seq.after(self.right_edge) {
      0
    } else {
      self.right_edge.diff(next_seq)
    }
  }

  pub fn set_size(&mut self, size: u32) {
    self.size = size;
    self.right_edge = self.left_edge + size;
  }

  pub fn size(&self) -> u32 {
    self.size
  }

  pub fn left_edge(&self) -> SeqNumber {
    self.left_edge
  }

  pub fn right_edge(&self) -> SeqNumber {
    self.right_edge
  }
}
