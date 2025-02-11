use anyhow::{Context, Result};
use clap::Parser;
use rsynx::local_sync::LocalSyncer;
use rsynx::network_sync::NetworkSyncer;
#[derive(Parser, Debug)]
#[command(author, about, long_about = None)]
struct Args {
    #[arg(short = 's', long = "server", default_value_t = false, help = "Run in server mode")]
    server: bool,

    #[arg(help = "Source path")]
    source: Option<String>,

    #[arg(help = "Destination path")]
    destination: Option<String>,

    #[arg(short = 'b', long = "block-size", default_value_t = 1024, help = "Block size used for synchronization (in bytes)")]
    block_size: usize,

    #[arg(short = 'm', long = "metadata", default_value_t = false, help = "Preserve file metadata (atime, mtime, permissions)")]
    preserve_metadata: bool,

    #[arg(short = 'd', long = "delete", default_value_t = false, help = "Delete extraneous files from destination directories")]
    delete_extraneous: bool,

    #[arg(short = 'p', long = "port", default_value_t = 7878, help = "Port number for server mode")]
    port: u16,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    if args.server {
        println!("Starting server on port {}", args.port);
        NetworkSyncer::serve(args.port, args.block_size)?;
    } else {
        let source = args.source.ok_or_else(|| anyhow::anyhow!("Source path required in client mode"))?;
        let destination = args.destination.ok_or_else(|| anyhow::anyhow!("Destination path required in client mode"))?;
        println!("Syncing {} to {}", source, destination);

        if destination.contains(":") {
            let parts = destination.split(":").collect::<Vec<&str>>();
            let syncer = NetworkSyncer::new(parts[0].to_string(), args.port, source, parts[1].to_string())
                .with_block_size(args.block_size);
            let result = syncer.sync().with_context(|| "Failed to sync")?;
            println!("Transferred: {} bytes, Not transferred: {} bytes", result.new_bytes, result.reused_bytes);
        } else {
            let syncer = LocalSyncer::new(source, destination)
                .with_block_size(args.block_size)
                .with_preserve_metadata(args.preserve_metadata)
                .with_delete_extraneous(args.delete_extraneous);
            let result = syncer.sync().with_context(|| "Failed to sync")?;
            println!("Transferred: {} bytes, Not transferred: {} bytes", result.new_bytes, result.reused_bytes);
        }
    }
    Ok(())
}

mod sync;
mod local_sync;
mod network_sync;