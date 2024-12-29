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
        signal: i32,
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
            id,
            bundle,
            pid_file,
            console_socket: _,
        } => {
            Container::create(&id, &bundle, &pid_file);
        }
        Command::Start { id } => {
            Container::start(&id);
        }
        Command::Kill { id, signal } => {
            Container::kill(&id, signal.try_into().unwrap());
        }
        Command::Delete { force, id } => {
            Container::delete(&id, force);
        }
    }
}
