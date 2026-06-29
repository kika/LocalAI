//! Out-of-order packet reassembly

use crate::utils::SeqNumber;
use std::collections::BTreeMap;

/// Buffer for reassembling out-of-order segments
pub struct ReorderBuffer {
  segments: BTreeMap<u32, Vec<u8>>,
  next_expected: SeqNumber,
  max_buffer_size: usize,
}

impl ReorderBuffer {
  pub fn new() -> Self {
    Self {
      segments: BTreeMap::new(),
      next_expected: SeqNumber(0),
      max_buffer_size: 1024 * 1024,
    }
  }

  pub fn add(&mut self, seq: SeqNumber, data: Vec<u8>) -> Vec<(SeqNumber, Vec<u8>)> {
    let mut ready = Vec::new();

    let seq_val = seq.0;
    if self.is_duplicate(seq, &data) {
      return ready;
    }

    if self.segments.len() * 1460 >= self.max_buffer_size {
      return ready;
    }

    self.segments.insert(seq_val, data);

    while let Some(data) = self.segments.remove(&self.next_expected.0) {
      let data_len = data.len() as u32;
      let seg_seq = self.next_expected;
      ready.push((seg_seq, data));
      self.next_expected = self.next_expected + data_len;
    }

    ready
  }

  fn is_duplicate(&self, seq: SeqNumber, _data: &[u8]) -> bool {
    if seq.before(self.next_expected) {
      return true;
    }

    let seq_val = seq.0;
    for (&start, existing_data) in self.segments.iter() {
      let existing_end = start.wrapping_add(existing_data.len() as u32);
      if seq_val < existing_end && start < seq_val + _data.len() as u32 {
        return true;
      }
    }

    false
  }

  pub fn set_next_expected(&mut self, seq: SeqNumber) {
    self.next_expected = seq;
  }

  pub fn next_expected(&self) -> SeqNumber {
    self.next_expected
  }

  pub fn clear(&mut self) {
    self.segments.clear();
  }
}

impl Default for ReorderBuffer {
  fn default() -> Self {
    Self::new()
  }
}
