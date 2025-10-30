use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use cdk_signatory::start_grpc_server;
use clap::Parser;
use tokio::sync::Mutex;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

use crate::signatory::TrezorSignatory;

mod mapping;
mod signatory;

#[derive(Parser)]
#[command(name = "cdk-signatory-trezor")]
#[command(version = "0.1.0")]
#[command(about = "Trezor Signatory CLI for Cashu CDK")]
struct Cli {
    #[arg(long, default_value = "127.0.0.1")]
    listen_addr: String,
    #[arg(long, default_value = "15060")]
    listen_port: u32,
    #[arg(long)]
    tls_dir: Option<PathBuf>,
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();
}

#[tokio::main]
pub async fn main() -> Result<()> {
    init_logging();

    let args: Cli = Cli::parse();

    let mut trezor = trezor_client::unique(false)?;
    trezor.init_device(None)?;

    let signatory = TrezorSignatory::new(Arc::new(Mutex::new(trezor))).await?;

    let socket_addr = SocketAddr::from_str(&format!("{}:{}", args.listen_addr, args.listen_port))?;

    start_grpc_server(Arc::new(signatory), socket_addr, args.tls_dir).await?;

    Ok(())
}
