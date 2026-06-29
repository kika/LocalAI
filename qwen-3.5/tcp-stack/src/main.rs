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

    info!("TCP Stack starting...");
    
    let data = vec![0x45u8, 0x00, 0x00, 0x28];
    let sum = calculate_checksum(&data);
    info!("Checksum test: {:04x}", sum);
    
    info!("TCP Stack initialized successfully");
    Ok(())
}
