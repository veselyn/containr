use std::collections::HashMap;
use std::io::Read;
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use anyhow::Context;
use nix::{sys::signal::Signal, unistd::Pid};
use oci_spec::runtime::Spec;
use serde::{Deserialize, Serialize};

use crate::process::Process;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Container {
    pub id: String,
    pub state: State,
}

impl Container {
    pub fn create(args: CreateArgs) -> anyhow::Result<Self> {
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

        let (pipe_fd_read, pipe_fd_write) = nix::unistd::pipe()?;
        let mut pipe_read = File::from(pipe_fd_read);
        let pipe_write = File::from(pipe_fd_write);

        let process = Process::new(container.clone(), spec, args.console_socket, pipe_write);
        let pid = process.spawn()?;
        fs::write(args.pid_file, pid.to_string().as_bytes())?;

        let mut buf = String::new();
        pipe_read.read_to_string(&mut buf)?;

        container.state.status = Status::Created;
        container.state.pid = Some(pid);
        container.save()?;

        Ok(container)
    }

    pub fn runtime_dir(&self) -> PathBuf {
        dirs::runtime_dir()
            .expect("unknown runtime dir")
            .join("containr")
            .join(&self.id)
    }

    pub fn load(id: &str) -> anyhow::Result<Self> {
        let mut container = Self {
            id: id.to_owned(),
            ..Self::default()
        };

        let state_file_path = container.runtime_dir().join("state.json");
        let state_file = File::open(state_file_path)?;

        let state: State = serde_json::from_reader(state_file)?;

        container.state = state;

        Ok(container)
    }

    pub fn state(&self) -> State {
        self.state.clone()
    }

    pub fn start(&self) -> anyhow::Result<()> {
        let start_fifo_path = self.runtime_dir().join("start");
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

        fs::remove_dir_all(self.runtime_dir())?;

        Ok(())
    }

    pub fn reload(&mut self) -> anyhow::Result<()> {
        let reloaded = Self::load(&self.id)?;
        *self = reloaded;
        Ok(())
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let runtime_dir = self.runtime_dir();
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
