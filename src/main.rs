mod cli;
mod container;
mod state;

use std::env::temp_dir;

use clap::Parser;
use cli::Cli;
use log::debug;

fn main() -> anyhow::Result<()> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!("[{}] {}", record.level(), message));
        })
        .chain(std::io::stderr())
        .chain(fern::log_file(temp_dir().join("containr.log"))?)
        .apply()?;

    debug!("received cli args {:?}", std::env::args());

    let cli = Cli::parse();
    cli.run()
}
