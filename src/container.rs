use std::collections::HashMap;
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use anyhow::Context;
use log::error;
use nix::sched::CloneCb;
use nix::{sched::CloneFlags, sys::signal::Signal, unistd::Pid};
use oci_spec::runtime::Spec;
use serde::{Deserialize, Serialize};

use crate::process::Process;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Container {
    id: String,
    state: State,
}

impl Container {
    pub fn create(args: CreateArgs) -> anyhow::Result<Container> {
        let config_file_path = format!("{}/config.json", args.bundle);
        let spec = Spec::load(config_file_path)?;

        let mut container = Self {
            id: args.id.clone(),
            state: State {
                oci_version: spec.version().to_owned(),
                id: args.id.clone(),
                status: Status::Creating,
                pid: None,
                bundle_path: args.bundle.to_owned(),
                annotations: spec.annotations().clone(),
            },
        };
        container.save()?;

        let pid = Self::spawn_process(&args.id, spec, args.console_socket)?;
        fs::write(args.pid_file, pid.to_string().as_bytes())?;

        container.state.status = Status::Created;
        container.state.pid = Some(pid);
        container.save()?;

        Ok(container)
    }

    fn spawn_process(id: &str, spec: Spec, console_socket: Option<String>) -> anyhow::Result<i32> {
        let runtime_dir = Self::runtime_dir(id)?;

        let callback: CloneCb = Box::new(|| {
            let process = Process {
                container_id: id.to_owned(),
                spec: spec.clone(),
                runtime_dir: runtime_dir.clone(),
                console_socket: console_socket.clone(),
            };

            match process.execute() {
                Ok(status) => status.code().unwrap().try_into().unwrap(),
                Err(err) => {
                    error!("process error: {}", err);
                    1
                }
            }
        });

        let mut stack = [0u8; 8192];

        let pid = unsafe { nix::sched::clone(callback, &mut stack, CloneFlags::empty(), None)? };

        Ok(pid.as_raw())
    }

    fn runtime_dir(id: &str) -> anyhow::Result<PathBuf> {
        Ok(dirs::runtime_dir()
            .context("unknown runtime dir")?
            .join("containr")
            .join(id))
    }

    pub fn load(id: &str) -> anyhow::Result<Container> {
        let state_file_path = Self::runtime_dir(id)?.join("state.json");
        let state_file = File::open(state_file_path)?;

        let state: State = serde_json::from_reader(state_file)?;

        Ok(Container {
            id: id.to_owned(),
            state,
        })
    }

    pub fn state(&self) -> State {
        self.state.clone()
    }

    pub fn start(&self) -> anyhow::Result<()> {
        let start_fifo_path = Self::runtime_dir(&self.id)?.join("start");
        let mut start_fifo = File::options().write(true).open(start_fifo_path)?;

        start_fifo.write_all(b"start")?;

        Ok(())
    }

    pub fn kill(&mut self, signal: Signal) -> anyhow::Result<()> {
        let status = &self.state().status;

        match status {
            Status::Creating | Status::Stopped => {
                anyhow::bail!("container is creating or stopped and can't be killed");
            }
            _ => {}
        }

        let pid = Pid::from_raw(self.state().pid.context("pid is required")?);
        nix::sys::signal::kill(pid, signal)?;

        self.state.pid = None;
        self.state.status = Status::Stopped;
        self.save()?;

        Ok(())
    }

    pub fn delete(self, force: bool) -> anyhow::Result<()> {
        if self.state.status != Status::Stopped && !force {
            if !force {
                anyhow::bail!("container is not stopped and can't be killed");
            }

            let pid = Pid::from_raw(self.state().pid.context("pid is required")?);
            nix::sys::signal::kill(pid, Signal::SIGKILL)?;
        }

        fs::remove_dir_all(Self::runtime_dir(&self.id)?)?;

        Ok(())
    }

    fn save(&self) -> anyhow::Result<()> {
        let runtime_dir = Self::runtime_dir(&self.id)?;
        fs::create_dir_all(&runtime_dir)?;

        let state_file_path = runtime_dir.join("state.json");
        let state_file = File::create(state_file_path)?;

        serde_json::to_writer_pretty(&state_file, &self.state)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub oci_version: String,
    pub id: String,
    pub status: Status,
    pub pid: Option<i32>,
    pub bundle_path: String,
    pub annotations: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Creating,
    Created,
    Running,
    Stopped,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CreateArgs {
    pub id: String,
    pub bundle: String,
    pub pid_file: String,
    pub console_socket: Option<String>,
}
