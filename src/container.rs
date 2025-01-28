use std::collections::HashMap;
use std::io::{Read, Seek};
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use anyhow::Context;
use nix::{sys::signal::Signal, unistd::Pid};
use oci_spec::runtime::Spec;
use serde::{Deserialize, Serialize};

use crate::sandbox::Sandbox;

#[derive(Debug)]
pub struct Container {
    pub id: String,
    pub state: State,
    state_file: File,
}

impl Container {
    pub fn create(args: CreateArgs) -> anyhow::Result<Self> {
        let config_file_path = format!("{}/config.json", args.bundle);
        let spec = Spec::load(config_file_path)?;

        let runtime_dir = Self::runtime_dir_self(&args.id);
        fs::create_dir_all(&runtime_dir)?;

        let state_file_path = runtime_dir.join("state.json");
        let state_file = File::create_new(state_file_path)?;

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
            state_file,
        };
        container.save()?;

        let (created_event_pipe_reader_fd, created_event_pipe_writer_fd) = nix::unistd::pipe()?;
        let mut created_event_pipe_reader = File::from(created_event_pipe_reader_fd);
        let created_event_pipe_writer = File::from(created_event_pipe_writer_fd);

        let sandbox = Sandbox::new(
            &mut container,
            spec,
            args.console_socket,
            created_event_pipe_writer,
        )?;
        let pid = sandbox.spawn()?;
        fs::write(args.pid_file, pid.to_string().as_bytes())?;

        let mut buf = String::new();
        created_event_pipe_reader.read_to_string(&mut buf)?;
        assert!(buf == "created");

        container.state.status = Status::Created;
        container.state.pid = Some(pid);
        container.save()?;

        Ok(container)
    }

    fn runtime_dir_self(id: &str) -> PathBuf {
        dirs::runtime_dir()
            .expect("unknown runtime dir")
            .join("containr")
            .join(id)
    }

    pub fn runtime_dir(&self) -> PathBuf {
        Self::runtime_dir_self(&self.id)
    }

    pub fn load(id: &str) -> anyhow::Result<Self> {
        let state_file_path = Self::runtime_dir_self(id).join("state.json");
        let state_file = File::open(state_file_path)?;

        let state: State = serde_json::from_reader(&state_file)?;

        Ok(Self {
            id: id.to_owned(),
            state,
            state_file,
        })
    }

    pub fn start(&self) -> anyhow::Result<()> {
        let start_fifo_path = self.runtime_dir().join("start");
        let mut start_fifo = File::options().write(true).open(start_fifo_path)?;

        start_fifo.write_all(b"start")?;

        Ok(())
    }

    pub fn kill(&mut self, signal: Signal) -> anyhow::Result<()> {
        let status = &self.state.status;

        match status {
            Status::Creating | Status::Stopped => {
                anyhow::bail!("container is creating or stopped and can't be killed");
            }
            _ => {}
        }

        let pid = Pid::from_raw(self.state.pid.context("pid is required")?);
        nix::sys::signal::kill(pid, signal)?;

        Ok(())
    }

    pub fn delete(self, force: bool) -> anyhow::Result<()> {
        if self.state.status != Status::Stopped && !force {
            if !force {
                anyhow::bail!("container is not stopped and can't be killed");
            }

            let pid = Pid::from_raw(self.state.pid.context("pid is required")?);
            nix::sys::signal::kill(pid, Signal::SIGKILL)?;
        }

        fs::remove_dir_all(self.runtime_dir())?;

        Ok(())
    }

    pub fn reload(&mut self) -> anyhow::Result<()> {
        self.state_file.rewind()?;

        self.state = serde_json::from_reader(&self.state_file)?;

        Ok(())
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        self.state_file.rewind()?;
        self.state_file.set_len(0)?;

        serde_json::to_writer_pretty(&self.state_file, &self.state)?;

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
