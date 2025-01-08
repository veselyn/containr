use std::{
    fs::{self, File},
    io::{Seek, Write},
    path::PathBuf,
};

use anyhow::Context;
use log::error;
use nix::{sched::CloneFlags, sys::signal::Signal, unistd::Pid};
use oci_spec::runtime::Spec;

use crate::{
    process::Process,
    state::{State, Status},
};

#[derive(Debug)]
pub struct Container {
    id: String,
    state: State,
}

impl Container {
    pub fn create(args: CreateArgs) -> anyhow::Result<()> {
        let config_file_path = format!("{}/config.json", args.bundle);
        let spec = Spec::load(config_file_path)?;

        let runtime_dir = Self::runtime_dir(&args.id)?;
        fs::create_dir_all(&runtime_dir)?;

        let state_file_path = runtime_dir.join("state.json");
        let mut state_file = File::create_new(state_file_path)?;

        let creating_state = State {
            oci_version: spec.version().to_owned(),
            id: args.id.to_owned(),
            status: Status::Creating,
            pid: None,
            bundle_path: args.bundle.to_owned(),
            annotations: spec.annotations().clone(),
        };

        serde_json::to_writer_pretty(&state_file, &creating_state)?;

        let pid = Self::spawn_process(&args.id, spec, args.console_socket)?;

        fs::write(args.pid_file, pid.to_string().as_bytes())?;

        let created_state = State {
            status: Status::Created,
            pid: Some(pid),
            ..creating_state
        };

        state_file.set_len(0)?;
        state_file.rewind()?;
        serde_json::to_writer_pretty(state_file, &created_state)?;

        Ok(())
    }

    fn spawn_process(id: &str, spec: Spec, console_socket: Option<String>) -> anyhow::Result<i32> {
        let process = Process {
            container_id: id.to_owned(),
            spec,
            runtime_dir: Self::runtime_dir(id)?,
            console_socket,
        };

        let mut stack = [0u8; 8192];

        let pid = unsafe {
            nix::sched::clone(
                Box::new(|| {
                    let process = process.clone();
                    match process.execute() {
                        Ok(status) => status.code().unwrap().try_into().unwrap(),
                        Err(err) => {
                            error!("process error: {}", err);
                            1
                        }
                    }
                }),
                &mut stack,
                CloneFlags::empty(),
                None,
            )?
        };

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

        let stopped_state = State {
            pid: None,
            status: Status::Stopped,
            ..self.state()
        };

        self.state = stopped_state;

        let state_file_path = Self::runtime_dir(&self.id)?.join("state.json");
        let state_file = File::options()
            .write(true)
            .truncate(true)
            .open(state_file_path)?;
        serde_json::to_writer_pretty(state_file, &self.state)?;

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
}

#[derive(Debug)]
pub struct CreateArgs {
    pub id: String,
    pub bundle: String,
    pub pid_file: String,
    pub console_socket: Option<String>,
}
