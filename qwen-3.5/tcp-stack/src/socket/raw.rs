//! Raw socket wrapper for Linux

use std::io;
use std::net::Ipv4Addr;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::prelude::*;
use tracing::trace;

/// Raw socket for sending/receiving IP packets
pub struct RawSocket {
  fd: OwnedFd,
}

impl RawSocket {
  /// Create a new raw socket
  pub fn new() -> io::Result<Self> {
    let fd =
      unsafe { libc::socket(libc::AF_INET, libc::SOCK_RAW, libc::IPPROTO_RAW) };

    if fd < 0 {
      return Err(io::Error::last_os_error());
    }

    let owned_fd = unsafe { OwnedFd::from_raw_fd(fd) };

    let socket = Self { fd: owned_fd };

    socket.set_iphdrincl()?;
    socket.set_broadcast()?;

    Ok(socket)
  }

  fn set_iphdrincl(&self) -> io::Result<()> {
    let value: libc::c_int = 1;
    let ret = unsafe {
      libc::setsockopt(
        self.fd.as_raw_fd(),
        libc::IPPROTO_IP,
        libc::IP_HDRINCL,
        &value as *const _ as *const libc::c_void,
        std::mem::size_of_val(&value) as libc::socklen_t,
      )
    };

    if ret < 0 {
      Err(io::Error::last_os_error())
    } else {
      Ok(())
    }
  }

  fn set_broadcast(&self) -> io::Result<()> {
    let value: libc::c_int = 1;
    let ret = unsafe {
      libc::setsockopt(
        self.fd.as_raw_fd(),
        libc::SOL_SOCKET,
        libc::SO_BROADCAST,
        &value as *const _ as *const libc::c_void,
        std::mem::size_of_val(&value) as libc::socklen_t,
      )
    };

    if ret < 0 {
      Err(io::Error::last_os_error())
    } else {
      Ok(())
    }
  }

  /// Send a packet to the given destination
  pub fn send_to(&self, packet: &[u8], dst: Ipv4Addr) -> io::Result<usize> {
    let mut addr = libc::sockaddr_in {
      sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
      sin_family: libc::AF_INET as libc::sa_family_t,
      sin_port: 0,
      sin_addr: libc::in_addr {
        s_addr: u32::from_ne_bytes(dst.octets()),
      },
      sin_zero: [0; 8],
    };

    let ret = unsafe {
      libc::sendto(
        self.fd.as_raw_fd(),
        packet.as_ptr() as *const libc::c_void,
        packet.len(),
        0,
        &mut addr as *mut _ as *mut libc::sockaddr,
        std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
      )
    };

    if ret < 0 {
      Err(io::Error::last_os_error())
    } else {
      trace!("Sent {} bytes to {}", ret, dst);
      Ok(ret as usize)
    }
  }

  /// Receive a packet
  pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, Ipv4Addr)> {
    let mut addr = libc::sockaddr_in {
      sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
      sin_family: libc::AF_INET as libc::sa_family_t,
      sin_port: 0,
      sin_addr: libc::in_addr { s_addr: 0 },
      sin_zero: [0; 8],
    };
    let mut addr_len = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;

    let ret = unsafe {
      libc::recvfrom(
        self.fd.as_raw_fd(),
        buf.as_mut_ptr() as *mut libc::c_void,
        buf.len(),
        0,
        &mut addr as *mut _ as *mut libc::sockaddr,
        &mut addr_len,
      )
    };

    if ret < 0 {
      Err(io::Error::last_os_error())
    } else {
      let src = Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr));
      trace!("Received {} bytes from {}", ret, src);
      Ok((ret as usize, src))
    }
  }

  /// Set non-blocking mode
  pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(self.fd.as_raw_fd(), libc::F_GETFL, 0) };
    if flags < 0 {
      return Err(io::Error::last_os_error());
    }

    let new_flags = if nonblocking {
      flags | libc::O_NONBLOCK
    } else {
      flags & !libc::O_NONBLOCK
    };

    let ret = unsafe { libc::fcntl(self.fd.as_raw_fd(), libc::F_SETFL, new_flags) };
    if ret < 0 {
      Err(io::Error::last_os_error())
    } else {
      Ok(())
    }
  }
}

impl AsRawFd for RawSocket {
  fn as_raw_fd(&self) -> RawFd {
    self.fd.as_raw_fd()
  }
}
