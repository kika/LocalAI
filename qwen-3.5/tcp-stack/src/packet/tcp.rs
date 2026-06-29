//! TCP header structure and options

use crate::utils::calculate_checksum;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::Cursor;

/// TCP flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpFlags(pub u8);

impl TcpFlags {
  pub const FIN: u8 = 0x01;
  pub const SYN: u8 = 0x02;
  pub const RST: u8 = 0x04;
  pub const PSH: u8 = 0x08;
  pub const ACK: u8 = 0x10;
  pub const URG: u8 = 0x20;
  pub const ECE: u8 = 0x40;
  pub const CWR: u8 = 0x80;

  pub fn new() -> Self {
    Self(0)
  }

  pub fn with_syn(mut self) -> Self {
    self.0 |= Self::SYN;
    self
  }

  pub fn with_ack(mut self) -> Self {
    self.0 |= Self::ACK;
    self
  }

  pub fn with_fin(mut self) -> Self {
    self.0 |= Self::FIN;
    self
  }

  pub fn with_rst(mut self) -> Self {
    self.0 |= Self::RST;
    self
  }

  pub fn with_psh(mut self) -> Self {
    self.0 |= Self::PSH;
    self
  }

  pub fn is_syn(&self) -> bool {
    (self.0 & Self::SYN) != 0
  }

  pub fn is_ack(&self) -> bool {
    (self.0 & Self::ACK) != 0
  }

  pub fn is_fin(&self) -> bool {
    (self.0 & Self::FIN) != 0
  }

  pub fn is_rst(&self) -> bool {
    (self.0 & Self::RST) != 0
  }

  pub fn is_syn_ack(&self) -> bool {
    self.is_syn() && self.is_ack() && (self.0 & !(Self::SYN | Self::ACK)) == 0
  }
}

impl Default for TcpFlags {
  fn default() -> Self {
    Self::new()
  }
}

/// TCP Options
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TcpOption {
  EndOfList,
  NoOperation,
  MaximumSegmentSize(u16),
  WindowScale(u8),
  SackPermitted,
  Sack { left: u32, right: u32 },
  Timestamp { ts_val: u32, ts_ecr: u32 },
}

impl TcpOption {
  pub const KIND_END: u8 = 0;
  pub const KIND_NOP: u8 = 1;
  pub const KIND_MSS: u8 = 2;
  pub const KIND_WINDOW_SCALE: u8 = 3;
  pub const KIND_SACK_PERMITTED: u8 = 4;
  pub const KIND_SACK: u8 = 5;
  pub const KIND_TIMESTAMP: u8 = 8;

  pub fn serialize(&self) -> Vec<u8> {
    match self {
      TcpOption::EndOfList => vec![Self::KIND_END],
      TcpOption::NoOperation => vec![Self::KIND_NOP],
      TcpOption::MaximumSegmentSize(mss) => {
        vec![Self::KIND_MSS, 4, (mss >> 8) as u8, (mss & 0xFF) as u8]
      }
      TcpOption::WindowScale(scale) => {
        vec![Self::KIND_WINDOW_SCALE, 3, *scale]
      }
      TcpOption::SackPermitted => {
        vec![Self::KIND_SACK_PERMITTED, 2]
      }
      TcpOption::Sack { left, right } => {
        let mut buf = vec![Self::KIND_SACK, 10];
        buf.extend_from_slice(&left.to_be_bytes());
        buf.extend_from_slice(&right.to_be_bytes());
        buf
      }
      TcpOption::Timestamp { ts_val, ts_ecr } => {
        let mut buf = vec![Self::KIND_TIMESTAMP, 10];
        buf.extend_from_slice(&ts_val.to_be_bytes());
        buf.extend_from_slice(&ts_ecr.to_be_bytes());
        buf
      }
    }
  }

  pub fn parse(data: &[u8]) -> Option<(Self, usize)> {
    if data.is_empty() {
      return None;
    }

    let kind = data[0];
    match kind {
      Self::KIND_END => Some((TcpOption::EndOfList, 1)),
      Self::KIND_NOP => Some((TcpOption::NoOperation, 1)),
      Self::KIND_MSS => {
        if data.len() < 4 {
          return None;
        }
        let len = data[1] as usize;
        if data.len() < len {
          return None;
        }
        let mss = u16::from_be_bytes([data[2], data[3]]);
        Some((TcpOption::MaximumSegmentSize(mss), len))
      }
      Self::KIND_WINDOW_SCALE => {
        if data.len() < 3 {
          return None;
        }
        Some((TcpOption::WindowScale(data[2]), 3))
      }
      Self::KIND_SACK_PERMITTED => {
        if data.len() < 2 {
          return None;
        }
        Some((TcpOption::SackPermitted, 2))
      }
      Self::KIND_TIMESTAMP => {
        if data.len() < 10 {
          return None;
        }
        let ts_val = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);
        let ts_ecr = u32::from_be_bytes([data[6], data[7], data[8], data[9]]);
        Some((TcpOption::Timestamp { ts_val, ts_ecr }, 10))
      }
      _ => {
        if data.len() < 2 {
          return None;
        }
        let len = data[1] as usize;
        if len < 2 || data.len() < len {
          return None;
        }
        Some((TcpOption::NoOperation, len))
      }
    }
  }
}

/// TCP Header
#[derive(Debug, Clone)]
pub struct TcpHeader {
  pub src_port: u16,
  pub dst_port: u16,
  pub seq_num: u32,
  pub ack_num: u32,
  pub data_offset: u8,
  pub flags: TcpFlags,
  pub window_size: u16,
  pub checksum: u16,
  pub urgent_pointer: u16,
  pub options: Vec<TcpOption>,
}

impl TcpHeader {
  pub const MIN_SIZE: usize = 20;

  pub fn new(src_port: u16, dst_port: u16) -> Self {
    Self {
      src_port,
      dst_port,
      seq_num: 0,
      ack_num: 0,
      data_offset: 5,
      flags: TcpFlags::new(),
      window_size: 65535,
      checksum: 0,
      urgent_pointer: 0,
      options: Vec::new(),
    }
  }

  pub fn syn(src_port: u16, dst_port: u16, seq_num: u32, mss: u16) -> Self {
    let mut header = Self::new(src_port, dst_port);
    header.seq_num = seq_num;
    header.flags = TcpFlags::new().with_syn();
    header.options = vec![
      TcpOption::MaximumSegmentSize(mss),
      TcpOption::SackPermitted,
      TcpOption::Timestamp {
        ts_val: 0,
        ts_ecr: 0,
      },
      TcpOption::WindowScale(7),
    ];
    header.data_offset = ((TcpHeader::MIN_SIZE + 12) / 4) as u8;
    header
  }

  pub fn syn_ack(
    src_port: u16,
    dst_port: u16,
    seq_num: u32,
    ack_num: u32,
    mss: u16,
  ) -> Self {
    let mut header = Self::syn(src_port, dst_port, seq_num, mss);
    header.flags = header.flags.with_ack();
    header.ack_num = ack_num;
    header
  }

  pub fn header_len(&self) -> usize {
    (self.data_offset as usize) * 4
  }

  pub fn options_len(&self) -> usize {
    self.header_len() - Self::MIN_SIZE
  }

  pub fn serialize(&self) -> Vec<u8> {
    let mut buf = Vec::with_capacity(self.header_len());

    buf.write_u16::<BigEndian>(self.src_port).unwrap();
    buf.write_u16::<BigEndian>(self.dst_port).unwrap();
    buf.write_u32::<BigEndian>(self.seq_num).unwrap();
    buf.write_u32::<BigEndian>(self.ack_num).unwrap();

    let data_offset_flags = ((self.data_offset as u16) << 4) | (self.flags.0 as u16);
    buf.write_u16::<BigEndian>(data_offset_flags).unwrap();

    buf.write_u16::<BigEndian>(self.window_size).unwrap();
    buf.write_u16::<BigEndian>(0).unwrap();
    buf.write_u16::<BigEndian>(self.urgent_pointer).unwrap();

    for option in &self.options {
      buf.extend(option.serialize());
    }

    while buf.len() < self.header_len() {
      buf.push(0);
    }

    buf
  }

  pub fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
    if data.len() < Self::MIN_SIZE {
      return None;
    }

    let mut cursor = Cursor::new(data);
    let src_port = cursor.read_u16::<BigEndian>().ok()?;
    let dst_port = cursor.read_u16::<BigEndian>().ok()?;
    let seq_num = cursor.read_u32::<BigEndian>().ok()?;
    let ack_num = cursor.read_u32::<BigEndian>().ok()?;
    let data_offset_flags = cursor.read_u16::<BigEndian>().ok()?;

    let data_offset = ((data_offset_flags >> 4) & 0x0F) as u8;
    let flags = (data_offset_flags & 0x3F) as u8;

    let window_size = cursor.read_u16::<BigEndian>().ok()?;
    let checksum = cursor.read_u16::<BigEndian>().ok()?;
    let urgent_pointer = cursor.read_u16::<BigEndian>().ok()?;

    let header_len = (data_offset as usize) * 4;
    if data.len() < header_len {
      return None;
    }

    let mut options = Vec::new();
    let options_data = &data[Self::MIN_SIZE..header_len];
    let mut offset = 0;

    while offset < options_data.len() {
      if let Some((option, len)) = TcpOption::parse(&options_data[offset..]) {
        if let TcpOption::EndOfList = option {
          break;
        }
        options.push(option);
        offset += len;
      } else {
        break;
      }
    }

    let payload = &data[header_len..];

    let header = Self {
      src_port,
      dst_port,
      seq_num,
      ack_num,
      data_offset,
      flags: TcpFlags(flags),
      window_size,
      checksum,
      urgent_pointer,
      options,
    };

    Some((header, payload))
  }

  pub fn calculate_checksum(
    &self,
    src_addr: u32,
    dst_addr: u32,
    payload: &[u8],
  ) -> u16 {
    let header_bytes = self.serialize();

    let mut pseudo_header = Vec::with_capacity(12);
    pseudo_header.extend_from_slice(&src_addr.to_be_bytes());
    pseudo_header.extend_from_slice(&dst_addr.to_be_bytes());
    pseudo_header.push(0);
    pseudo_header.push(6);
    let tcp_len = (header_bytes.len() + payload.len()) as u16;
    pseudo_header.extend_from_slice(&tcp_len.to_be_bytes());

    let mut total = pseudo_header;
    total.extend(header_bytes);
    total.extend_from_slice(payload);

    calculate_checksum(&total)
  }
}
