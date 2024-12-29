use std::{
    collections::HashMap,
    io::{Read, Seek, Write},
};

use log::{debug, trace};
use nix::{
    sched::CloneFlags,
    sys::{signal::Signal, stat::Mode},
    unistd::Pid,
};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Container {}

impl Container {
    pub fn state(id: &str) -> State {
        let container_runtime_dir = dirs::runtime_dir().unwrap().join("containr").join(id);
        let state_file = std::fs::File::open(container_runtime_dir.join("state.json")).unwrap();
        serde_json::from_reader(state_file).unwrap()
    }

    pub fn create(id: &str, bundle: &str, pid_file: &str) {
        let spec = oci_spec::runtime::Spec::load(format!("{bundle}/config.json")).unwrap();
        trace!(spec:?; "loaded oci runtime spec");

        let container_runtime_dir = dirs::runtime_dir().unwrap().join("containr").join(id);
        std::fs::create_dir_all(&container_runtime_dir).unwrap();

        let mut state_file =
            std::fs::File::create_new(container_runtime_dir.join("state.json")).unwrap();

        let mut state = State {
            oci_version: spec.version().to_owned(),
            id: id.to_owned(),
            status: Status::Creating,
            pid: None,
            bundle_path: bundle.to_owned(),
            annotations: spec.annotations().clone(),
        };

        serde_json::to_writer_pretty(&state_file, &state).unwrap();

        let mut stack = [0u8; 8192];

        let child_pid = unsafe {
            let pid = nix::sched::clone(
                Box::new(|| {
                    let start_pipe = container_runtime_dir.join("start");
                    nix::unistd::mkfifo(&start_pipe, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();

                    let mut start_pipe_reader = std::fs::File::open(start_pipe).unwrap();

                    let mut buf = String::new();
                    start_pipe_reader.read_to_string(&mut buf).unwrap();

                    let mut args = spec
                        .process()
                        .as_ref()
                        .unwrap()
                        .args()
                        .as_ref()
                        .unwrap()
                        .iter();

                    let status = std::process::Command::new(args.next().unwrap())
                        .args(args)
                        .status()
                        .unwrap();

                    status.code().unwrap().try_into().unwrap()
                }),
                &mut stack,
                CloneFlags::empty(),
                None,
            )
            .unwrap();
            pid.as_raw()
        };

        debug!(pid = child_pid.to_string().as_str(); "started container");

        let mut pid_file = std::fs::File::create_new(pid_file).unwrap();
        pid_file
            .write_all(child_pid.to_string().as_bytes())
            .unwrap();

        state.status = Status::Created;
        state.pid = Some(child_pid);

        state_file.set_len(0).unwrap();
        state_file.rewind().unwrap();
        serde_json::to_writer_pretty(state_file, &state).unwrap();
    }

    pub fn start(id: &str) {
        let container_runtime_dir = dirs::runtime_dir().unwrap().join("containr").join(id);

        let start_pipe = container_runtime_dir.join("start");

        let mut start_pipe_writer = std::fs::File::options()
            .write(true)
            .open(start_pipe)
            .unwrap();

        start_pipe_writer.write_all(b"start container").unwrap();
    }

    pub fn kill(id: &str, signal: Signal) {
        let container_runtime_dir = dirs::runtime_dir().unwrap().join("containr").join(id);

        let state: State = serde_json::from_reader(
            std::fs::File::open(container_runtime_dir.join("state.json")).unwrap(),
        )
        .unwrap();

        match state.status {
            Status::Creating | Status::Stopped => {
                panic!("container is not running")
            }
            _ => {}
        }

        let pid = state.pid.unwrap();
        nix::sys::signal::kill(Pid::from_raw(pid), signal).unwrap();
        debug!(id, pid; "killed container");
    }

    pub fn delete(id: &str, force: bool) {
        let container_runtime_dir = dirs::runtime_dir().unwrap().join("containr").join(id);

        let state: State = serde_json::from_reader(
            std::fs::File::open(container_runtime_dir.join("state.json")).unwrap(),
        )
        .unwrap();

        match state.status {
            Status::Created | Status::Running => {
                if !force {
                    panic!("container process is running; can't force delete")
                }

                let pid = state.pid.unwrap();
                nix::sys::signal::kill(Pid::from_raw(pid), Signal::SIGKILL).unwrap();
                debug!(id, pid; "killed running container");
            }
            _ => {}
        }

        std::fs::remove_dir_all(container_runtime_dir).unwrap();

        debug!(id; "deleted container");
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    oci_version: String,
    id: String,
    status: Status,
    pid: Option<i32>,
    bundle_path: String,
    annotations: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Creating,
    Created,
    Running,
    Stopped,
}
