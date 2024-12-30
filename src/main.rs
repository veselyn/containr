mod cli;
mod container;

use clap::Parser;
use cli::Cli;
use log::debug;

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    debug!(args:? = std::env::args(); "received cli args");

    let cli = Cli::parse();
    cli.run();
}
