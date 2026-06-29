//! TCP Stack - A userspace TCP implementation using raw sockets
//!
//! This crate provides a complete TCP protocol implementation that runs in userspace
//! using Linux raw IP sockets. It includes:
//!
//! - Full TCP state machine (RFC 793)
//! - Congestion control (NewReno)
//! - Flow control with sliding windows
//! - Retransmission with dynamic RTO calculation
//! - Selective Acknowledgments (SACK)
//! - TCP options (MSS, Window Scaling, Timestamps)

pub mod packet;
pub mod socket;
pub mod connection;
pub mod reliability;
pub mod flow_control;
pub mod congestion;
pub mod demux;
pub mod utils;

pub use connection::TcpConnection;
pub use socket::RawSocket;
