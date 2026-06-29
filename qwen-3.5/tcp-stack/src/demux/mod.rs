//! Packet demultiplexing

use crate::packet::{Ipv4Header, TcpHeader};
use std::collections::HashMap;
use std::net::SocketAddrV4;

/// Demultiplexer for routing packets to connections
pub struct Demultiplexer {
  connections: HashMap<ConnectionKey, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConnectionKey {
  pub local: SocketAddrV4,
  pub remote: SocketAddrV4,
}

impl ConnectionKey {
  pub fn new(local: SocketAddrV4, remote: SocketAddrV4) -> Self {
    Self { local, remote }
  }

  pub fn from_headers(ip: &Ipv4Header, tcp: &TcpHeader) -> Option<Self> {
    Some(Self {
      local: SocketAddrV4::new(ip.dst_addr, tcp.dst_port),
      remote: SocketAddrV4::new(ip.src_addr, tcp.src_port),
    })
  }
}

impl Demultiplexer {
  pub fn new() -> Self {
    Self {
      connections: HashMap::new(),
    }
  }

  pub fn register(&mut self, key: ConnectionKey, id: u64) {
    self.connections.insert(key, id);
  }

  pub fn unregister(&mut self, key: &ConnectionKey) {
    self.connections.remove(key);
  }

  pub fn find(&self, key: &ConnectionKey) -> Option<&u64> {
    self.connections.get(key)
  }
}

impl Default for Demultiplexer {
  fn default() -> Self {
    Self::new()
  }
}
