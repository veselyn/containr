use crate::container::{Container, CreateArgs};
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
    pub fn run(self) -> anyhow::Result<()> {
        match self.command {
            Command::State { id } => {
                let state = Container::load(&id)?.state();
                println!("{}", serde_json::to_string_pretty(&state)?);
                Ok(())
            }
            Command::Create {
                id,
                bundle,
                pid_file,
                console_socket,
            } => Container::create(CreateArgs {
                id,
                bundle,
                pid_file,
                console_socket,
            }),
            Command::Start { id } => Container::load(&id)?.start(),
            Command::Kill { id, signal } => Container::load(&id)?.kill(signal.try_into()?),
            Command::Delete { force, id } => Container::load(&id)?.delete(force),
        }
    }
}
