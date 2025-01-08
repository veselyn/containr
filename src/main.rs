mod cli;
mod container;
mod logging;
mod process;

use clap::Parser;
use cli::Cli;
use log::debug;

fn main() -> anyhow::Result<()> {
    logging::init();

    debug!("received cli args {:?}", std::env::args());

    let cli = Cli::parse();
    cli.run()
}
