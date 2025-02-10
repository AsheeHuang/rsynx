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
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    println!("Syncing {} to {}", args.source, args.destination);

    let syncer = Syncer::new(args.source, args.destination);
    syncer.sync()?;

    Ok(())
}

mod sync;