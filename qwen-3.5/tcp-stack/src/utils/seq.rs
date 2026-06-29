//! TCP sequence number arithmetic

use std::ops::{Add, Sub};

/// TCP sequence number (32-bit, wraps around)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeqNumber(pub u32);

impl SeqNumber {
  pub fn new(val: u32) -> Self {
    Self(val)
  }

  pub fn random() -> Self {
    use rand::Rng;
    Self(rand::thread_rng().gen_range(0..=u32::MAX))
  }

  pub fn wrapping_add(self, other: u32) -> Self {
    Self(self.0.wrapping_add(other))
  }

  pub fn wrapping_sub(self, other: u32) -> Self {
    Self(self.0.wrapping_sub(other))
  }

  pub fn diff(self, other: SeqNumber) -> u32 {
    self.0.wrapping_sub(other.0)
  }

  pub fn before(self, other: SeqNumber) -> bool {
    self.diff(other) > 0x8000_0000
  }

  pub fn after(self, other: SeqNumber) -> bool {
    other.before(self)
  }
}

impl Add<u32> for SeqNumber {
  type Output = SeqNumber;

  fn add(self, other: u32) -> Self::Output {
    self.wrapping_add(other)
  }
}

impl Sub<u32> for SeqNumber {
  type Output = SeqNumber;

  fn sub(self, other: u32) -> Self::Output {
    self.wrapping_sub(other)
  }
}

impl Sub<SeqNumber> for SeqNumber {
  type Output = u32;

  fn sub(self, other: SeqNumber) -> Self::Output {
    self.diff(other)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_seq_wrap() {
    let seq = SeqNumber(u32::MAX);
    assert_eq!((seq + 1).0, 0);
  }

  #[test]
  fn test_seq_before() {
    let seq1 = SeqNumber(100);
    let seq2 = SeqNumber(200);
    assert!(seq1.before(seq2));
    assert!(!seq2.before(seq1));
  }
}
