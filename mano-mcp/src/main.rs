use mano_mcp::cli::{parse_binary_path, validate_binary};
use mano_mcp::server::ManoMcp;
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mano_bin = parse_binary_path(std::env::args());

    if let Err(e) = validate_binary(&mano_bin) {
        eprintln!("{}", e);
        std::process::exit(1);
    }

    let service = ManoMcp::new(mano_bin).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
