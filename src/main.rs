use clap::Parser;
use punct_convert::{AppArgs, run};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = AppArgs::parse();
    run(args);
}
