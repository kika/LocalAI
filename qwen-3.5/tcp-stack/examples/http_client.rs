//! TCP HTTP Client Example
//!
//! This example demonstrates making a simple HTTP request using our userspace TCP stack.
//! Note: Requires root privileges to create raw sockets.

use tcp_stack::utils::calculate_checksum;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("tcp_stack=info".parse()?),
        )
        .init();

    info!("TCP HTTP Client example");
    
    // Test checksum calculation
    let data = vec![0x45u8, 0x00, 0x00, 0x28, 0x00, 0x00, 0x40, 0x00];
    let checksum = calculate_checksum(&data);
    info!("Test checksum: 0x{:04x}", checksum);
    
    info!("HTTP client would connect to example.com:80");
    info!("Note: Full implementation requires completing the TCP state machine");
    
    Ok(())
}
