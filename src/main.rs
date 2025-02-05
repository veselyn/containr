mod cli;
mod container;
mod logging;
mod process;
mod sandbox;

use clap::Parser;
use cli::Cli;

fn main() -> anyhow::Result<()> {
    logging::init();

    log::trace!("received cli args {:?}", std::env::args());

    let cli = Cli::parse();
    cli.run()
}
