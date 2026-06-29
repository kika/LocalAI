//! TCP connection states

/// TCP connection states (RFC 793)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TcpState {
  #[default]
  Closed,
  Listen,
  SynSent,
  SynReceived,
  Established,
  FinWait1,
  FinWait2,
  CloseWait,
  Closing,
  LastAck,
  TimeWait,
}

impl TcpState {
  pub fn is_closed(&self) -> bool {
    matches!(self, Self::Closed | Self::TimeWait)
  }

  pub fn is_established(&self) -> bool {
    matches!(self, Self::Established)
  }

  pub fn is_syn_sent(&self) -> bool {
    matches!(self, Self::SynSent)
  }
}
