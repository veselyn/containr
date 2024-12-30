mod cli;
mod container;
mod state;

use clap::Parser;
use cli::Cli;
use log::debug;

fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    debug!(args:? = std::env::args(); "received cli args");

    let cli = Cli::parse();
    cli.run()
}
