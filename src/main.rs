mod cli;
mod container;
mod logging;
mod process;
mod sandbox;

use clap::Parser;
use cli::Cli;
use log::trace;

fn main() -> anyhow::Result<()> {
    logging::init();

    trace!("received cli args {:?}", std::env::args());

    let cli = Cli::parse();
    cli.run()
}
