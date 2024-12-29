use std::{
    collections::HashMap,
    io::{Seek, Write},
};

use log::{debug, trace};
use nix::sched::CloneFlags;
use serde::Serialize;

#[derive(Debug)]
pub struct Container {}

impl Container {
    pub fn create(bundle: &str, pid_file: &str, id: &str) {
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
            let pid = nix::sched::clone(Box::new(&process), &mut stack, CloneFlags::empty(), None)
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
}

#[derive(Debug, Serialize)]
struct State {
    oci_version: String,
    id: String,
    status: Status,
    pid: Option<i32>,
    bundle_path: String,
    annotations: Option<HashMap<String, String>>,
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
