//! IPv4 header structure

use crate::utils::calculate_checksum;
use byteorder::{BigEndian, WriteBytesExt};
use std::net::Ipv4Addr;

/// IPv4 header (20 bytes minimum)
#[derive(Debug, Clone)]
pub struct Ipv4Header {
  pub version: u8,
  pub ihl: u8,
  pub dscp: u8,
  pub ecn: u8,
  pub total_length: u16,
  pub identification: u16,
  pub flags: u8,
  pub fragment_offset: u16,
  pub ttl: u8,
  pub protocol: u8,
  pub checksum: u16,
  pub src_addr: Ipv4Addr,
  pub dst_addr: Ipv4Addr,
  pub options: Vec<u8>,
}

impl Ipv4Header {
  pub const MIN_SIZE: usize = 20;
  pub const VERSION: u8 = 4;
  pub const PROTOCOL_TCP: u8 = 6;

  pub fn new(src_addr: Ipv4Addr, dst_addr: Ipv4Addr, payload_len: usize) -> Self {
    Self {
      version: Self::VERSION,
      ihl: 5,
      dscp: 0,
      ecn: 0,
      total_length: (Self::MIN_SIZE + payload_len) as u16,
      identification: 0,
      flags: 0x40,
      fragment_offset: 0,
      ttl: 64,
      protocol: Self::PROTOCOL_TCP,
      checksum: 0,
      src_addr,
      dst_addr,
      options: Vec::new(),
    }
  }

  pub fn header_len(&self) -> usize {
    (self.ihl as usize) * 4
  }

  pub fn serialize(&self) -> Vec<u8> {
    let mut buf = Vec::with_capacity(self.header_len());

    let version_ihl = (self.version << 4) | self.ihl;
    let dscp_ecn = (self.dscp << 2) | (self.ecn & 0x03);

    buf.write_u8(version_ihl).unwrap();
    buf.write_u8(dscp_ecn).unwrap();
    buf.write_u16::<BigEndian>(self.total_length).unwrap();
    buf.write_u16::<BigEndian>(self.identification).unwrap();

    let flags_frag = ((self.flags as u16) << 13) | (self.fragment_offset & 0x1FFF);
    buf.write_u16::<BigEndian>(flags_frag).unwrap();

    buf.write_u8(self.ttl).unwrap();
    buf.write_u8(self.protocol).unwrap();
    buf.write_u16::<BigEndian>(0).unwrap();

    buf.extend_from_slice(&self.src_addr.octets());
    buf.extend_from_slice(&self.dst_addr.octets());

    if !self.options.is_empty() {
      buf.extend_from_slice(&self.options);
    }

    let checksum = calculate_checksum(&buf);

    buf[10] = (checksum >> 8) as u8;
    buf[11] = (checksum & 0xFF) as u8;

    buf
  }

  pub fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
    if data.len() < Self::MIN_SIZE {
      return None;
    }

    let version = (data[0] >> 4) & 0x0F;
    if version != 4 {
      return None;
    }

    let ihl = data[0] & 0x0F;
    let header_len = (ihl as usize) * 4;

    if data.len() < header_len {
      return None;
    }

    let dscp_ecn = data[1];
    let total_length = u16::from_be_bytes([data[2], data[3]]);
    let identification = u16::from_be_bytes([data[4], data[5]]);
    let flags_frag = u16::from_be_bytes([data[6], data[7]]);
    let flags = ((flags_frag >> 13) & 0x07) as u8;
    let fragment_offset = flags_frag & 0x1FFF;
    let ttl = data[8];
    let protocol = data[9];

    let src_addr = Ipv4Addr::new(data[12], data[13], data[14], data[15]);
    let dst_addr = Ipv4Addr::new(data[16], data[17], data[18], data[19]);

    let options = if header_len > Self::MIN_SIZE {
      data[Self::MIN_SIZE..header_len].to_vec()
    } else {
      Vec::new()
    };

    let header = Self {
      version,
      ihl,
      dscp: dscp_ecn >> 2,
      ecn: dscp_ecn & 0x03,
      total_length,
      identification,
      flags,
      fragment_offset,
      ttl,
      protocol,
      checksum: u16::from_be_bytes([data[10], data[11]]),
      src_addr,
      dst_addr,
      options,
    };

    Some((header, &data[header_len..]))
  }
}
