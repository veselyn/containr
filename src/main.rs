mod container;

use clap::{Parser, Subcommand};
use container::Container;
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
        .filter_level(log::LevelFilter::Debug)
        .init();

    debug!(args:? = std::env::args(); "received cli args");

    let cli = Cli::parse();

    match cli.command {
        Command::State { id: _ } => todo!(),
        Command::Create {
            bundle,
            pid_file,
            console_socket: _,
            id,
        } => {
            let spec = oci_spec::runtime::Spec::load(format!("{bundle}/config.json")).unwrap();
            trace!(spec:?; "loaded oci runtime spec");

            let container = Container {
                spec,
                bundle,
                pid_file,
                id,
            };

            container.create();
        }
        Command::Start { id: _ } => todo!(),
        Command::Kill { id: _ } => todo!(),
        Command::Delete { force: _, id: _ } => {}
    }
}
