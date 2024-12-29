mod container;

use clap::{Parser, Subcommand};
use container::Container;
use log::debug;

#[derive(Debug, Parser)]
struct Cli {
    #[arg(long)]
    systemd_cgroup: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    State {
        id: String,
    },
    Create {
        id: String,

        #[arg(long)]
        bundle: String,

        #[arg(long)]
        pid_file: String,

        #[arg(long)]
        console_socket: Option<String>,
    },
    Start {
        id: String,
    },
    Kill {
        id: String,
    },
    Delete {
        #[arg(long)]
        force: bool,

        id: String,
    },
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    debug!(args:? = std::env::args(); "received cli args");

    let cli = Cli::parse();

    match cli.command {
        Command::State { id } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&Container::state(&id)).unwrap()
            );
        }
        Command::Create {
            bundle,
            pid_file,
            console_socket: _,
            id,
        } => {
            Container::create(&bundle, &pid_file, &id);
        }
        Command::Start { id: _ } => todo!(),
        Command::Kill { id: _ } => todo!(),
        Command::Delete { force, id } => {
            Container::delete(&id, force);
        }
    }
}
