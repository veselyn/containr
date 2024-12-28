use std::{
    collections::HashMap,
    io::{Seek, Write},
};

use clap::{Parser, Subcommand};
use log::{debug, trace};
use nix::sched::CloneFlags;
use oci_spec::runtime::Spec;
use serde::Serialize;

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

#[derive(Debug)]
struct Container {
    spec: Spec,
    bundle: String,
    pid_file: String,
    id: String,
}

impl Container {
    fn create(&self) {
        let container_runtime_dir = dirs::runtime_dir().unwrap().join("containr").join(&self.id);
        std::fs::create_dir_all(&container_runtime_dir).unwrap();

        let mut state_file =
            std::fs::File::create_new(container_runtime_dir.join("state.json")).unwrap();

        let mut state = State {
            oci_version: self.spec.version().to_owned(),
            id: self.id.clone(),
            status: Status::Creating,
            pid: None,
            bundle_path: self.bundle.clone(),
            annotations: self.spec.annotations().clone().unwrap_or_default(),
        };

        serde_json::to_writer_pretty(&state_file, &state).unwrap();

        let mut stack = [0u8; 8192];

        let child_pid = unsafe {
            let pid = nix::sched::clone(Box::new(&process), &mut stack, CloneFlags::empty(), None)
                .unwrap();
            pid.as_raw()
        };

        debug!(pid = child_pid.to_string().as_str(); "started container");

        let mut pid_file = std::fs::File::create_new(&self.pid_file).unwrap();
        pid_file
            .write_all(child_pid.to_string().as_bytes())
            .unwrap();

        state.status = Status::Created;
        state.pid = Some(child_pid);

        state_file.set_len(0).unwrap();
        state_file.rewind().unwrap();
        serde_json::to_writer_pretty(state_file, &state).unwrap();
    }
}

#[derive(Debug, Serialize)]
struct State {
    oci_version: String,
    id: String,
    status: Status,
    pid: Option<i32>,
    bundle_path: String,
    annotations: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Creating,
    Created,
    // Running,
    // Stopped,
}

fn process() -> isize {
    0
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
