//! TCP/IP checksum calculation

/// Trait for types that can calculate their checksum
pub trait CalculateChecksum {
  fn calculate_checksum(&self) -> u16;
}

/// Calculate TCP/IP checksum over data
///
/// The checksum is the 16-bit one's complement of the one's complement sum
/// of all 16-bit words. If the data has an odd number of bytes, the last
/// byte is padded with zeros for checksum calculation.
pub fn calculate_checksum(data: &[u8]) -> u16 {
  let mut sum = 0u32;
  let mut i = 0;

  while i < data.len() {
    if i + 1 < data.len() {
      let word = ((data[i] as u32) << 8) | (data[i + 1] as u32);
      sum += word;
      i += 2;
    } else {
      let word = (data[i] as u32) << 8;
      sum += word;
      i += 1;
    }
  }

  while (sum & 0xFFFF_0000) != 0 {
    sum = (sum & 0xFFFF) + (sum >> 16);
  }

  !sum as u16
}

/// Calculate pseudo-header checksum for TCP
pub fn calculate_pseudo_header_checksum(
  src_addr: u32,
  dst_addr: u32,
  protocol: u8,
  tcp_length: u16,
) -> u16 {
  let mut sum = 0u32;

  sum += src_addr >> 16;
  sum += src_addr & 0xFFFF;
  sum += dst_addr >> 16;
  sum += dst_addr & 0xFFFF;
  sum += protocol as u32;
  sum += tcp_length as u32;

  while (sum & 0xFFFF_0000) != 0 {
    sum = (sum & 0xFFFF) + (sum >> 16);
  }

  sum as u16
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_basic_checksum() {
    let data = [0x45u8, 0x00, 0x00, 0x28];
    let sum = calculate_checksum(&data);
    assert_ne!(sum, 0);
  }
}
