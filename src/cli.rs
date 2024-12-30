use crate::container::Container;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
pub struct Cli {
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

impl Cli {
    pub fn run(self) {
        match self.command {
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
}
