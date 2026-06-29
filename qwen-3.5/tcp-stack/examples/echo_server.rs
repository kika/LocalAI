//! TCP Echo Server Example
//!
//! This example demonstrates a simple TCP echo server using our userspace TCP stack.
//! Note: Requires root privileges to create raw sockets.

use tcp_stack::{RawSocket, TcpConnection};
use tracing::{info, error};
use std::net::{Ipv4Addr, SocketAddrV4};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("tcp_stack=debug".parse()?),
        )
        .init();

    info!("TCP Echo Server starting...");

    // Try to create a raw socket (requires root)
    match RawSocket::new() {
        Ok(socket) => {
            info!("Raw socket created successfully");
            
            let local_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080);
            let remote_addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 12345);
            
            let _conn = TcpConnection::new(socket, local_addr, remote_addr);
            info!("TCP connection initialized");
            
            info!("Echo server ready on port 8080");
            info!("Note: Full implementation requires packet processing loop");
        }
        Err(e) => {
            error!("Failed to create raw socket: {}", e);
            info!("Run with: sudo -E $(which cargo) run --example echo_server");
        }
    }

    Ok(())
}
