use anyhow::Result;
use clap::Parser;
use crate::sync::Syncer;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    source: String,

    destination: String,

    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    #[arg(short = 'b', long = "block-size", default_value_t = 1024, help = "Block size used for synchronization (in bytes)")]
    block_size: usize,

    #[arg(short = 'm', long = "metadata", default_value_t = false, help = "Preserve file metadata (atime, mtime, permissions)")]
    preserve_metadata: bool,

    #[arg(short = 'd', long = "delete", default_value_t = false, help = "Delete extraneous files from destination directories")]
    delete_extraneous: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    println!("Syncing {} to {}", args.source, args.destination);

    let syncer = Syncer::new(args.source, args.destination)
        .with_block_size(args.block_size)
        .with_preserve_metadata(args.preserve_metadata)
        .with_delete_extraneous(args.delete_extraneous);
    let result = syncer.sync()?;
    println!("Transferred: {} bytes, Not transferred: {} bytes", result.new_bytes, result.reused_bytes);

    Ok(())
}

mod sync;