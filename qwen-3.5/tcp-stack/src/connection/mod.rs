//! TCP connection state machine

pub mod control;
pub mod states;
pub mod timer;

pub use control::ControlBlock;
pub use states::TcpState;
pub use timer::Timer;

use crate::socket::RawSocket;
use std::net::SocketAddrV4;
use tracing::debug;

/// TCP Connection
pub struct TcpConnection {
  pub control: ControlBlock,
  pub socket: RawSocket,
  pub remote: SocketAddrV4,
  pub local: SocketAddrV4,
}

impl TcpConnection {
  pub fn new(socket: RawSocket, local: SocketAddrV4, remote: SocketAddrV4) -> Self {
    Self {
      control: ControlBlock::new(),
      socket,
      remote,
      local,
    }
  }

  pub fn state(&self) -> TcpState {
    self.control.state
  }

  pub fn set_state(&mut self, state: TcpState) {
    debug!("State transition: {:?} -> {:?}", self.control.state, state);
    self.control.state = state;
  }
}
