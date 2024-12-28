use std::io::Write;

use clap::{Parser, Subcommand};
use log::{debug, trace};

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
        #[arg(long)]
        bundle: String,

        #[arg(long)]
        pid_file: String,

        #[arg(long)]
        console_socket: Option<String>,

        id: String,
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
        .filter_level(log::LevelFilter::Trace)
        .init();

    trace!(args:? = std::env::args(); "received cli args");

    let cli = Cli::parse();

    match cli.command {
        Command::State { id: _ } => todo!(),
        Command::Create {
            bundle: _,
            pid_file,
            console_socket: _,
            id: _,
        } => {
            #[allow(clippy::zombie_processes)]
            let child = std::process::Command::new("bash").spawn().unwrap();
            debug!(pid = child.id(); "started container");

            let mut pid_file = std::fs::File::create_new(pid_file).unwrap();
            pid_file
                .write_all(child.id().to_string().as_bytes())
                .unwrap();
        }
        Command::Start { id: _ } => todo!(),
        Command::Kill { id: _ } => todo!(),
        Command::Delete { force: _, id: _ } => {}
    }
}
