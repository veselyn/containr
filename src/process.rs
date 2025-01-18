use std::{
    fs::File,
    io::{IoSlice, Read, Write},
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd},
        unix::net::UnixStream,
    },
    process::{Command, ExitStatus},
};

use anyhow::Context;
use log::error;
use nix::{
    libc,
    pty::OpenptyResult,
    sched::{CloneCb, CloneFlags},
    sys::{
        socket::{ControlMessage, MsgFlags},
        stat::Mode,
    },
};
use oci_spec::runtime::Spec;
use serde_json::json;

use crate::container::{Container, Status};

#[derive(Debug)]
pub struct Process {
    container: Container,
    spec: Spec,
    console_socket: Option<String>,
    pipe_write: File,
}

impl Process {
    pub fn new(
        container: Container,
        spec: Spec,
        console_socket: Option<String>,
        pipe_write: File,
    ) -> Self {
        Self {
            container,
            spec,
            console_socket,
            pipe_write,
        }
    }

    pub fn spawn(mut self) -> anyhow::Result<i32> {
        let callback: CloneCb = Box::new(|| match self.execute() {
            Ok(status) => status.code().unwrap().try_into().unwrap(),
            Err(err) => {
                error!("process error: {}", err);
                1
            }
        });

        let mut stack = [0u8; 8192];

        let pid = unsafe { nix::sched::clone(callback, &mut stack, CloneFlags::empty(), None)? };

        Ok(pid.as_raw())
    }

    fn execute(&mut self) -> anyhow::Result<ExitStatus> {
        let pty = self
            .console_socket
            .as_ref()
            .map(|console_socket| -> anyhow::Result<OpenptyResult> {
                let pty = nix::pty::openpty(None, None)?;
                self.pass_pty_master(console_socket, pty.master.as_fd())?;
                Ok(pty)
            })
            .transpose()?;

        let start_fifo_path = self.container.runtime_dir().join("start");
        nix::unistd::mkfifo(&start_fifo_path, Mode::S_IRUSR | Mode::S_IWUSR)?;

        let spec_process = self.spec.process().as_ref().context("no process in spec")?;

        let mut args = spec_process
            .args()
            .as_ref()
            .context("no process args in spec")?
            .iter();

        if let Some(pty) = pty {
            nix::unistd::setsid()?;
            let ret = unsafe { libc::ioctl(pty.slave.as_raw_fd(), libc::TIOCSCTTY, 0) };
            assert!(ret == 0);

            nix::unistd::dup2(pty.slave.as_raw_fd(), 0)?;
            nix::unistd::dup2(pty.slave.as_raw_fd(), 1)?;
            nix::unistd::dup2(pty.slave.as_raw_fd(), 2)?;
        }

        let mut process = Command::new(args.next().context("process args are empty")?);
        process.args(args);
        process.env_clear();
        process.envs(
            spec_process
                .env()
                .clone()
                .unwrap_or_default()
                .iter()
                .map(|e| e.split_once("=").unwrap()),
        );

        self.pipe_write.write_all(b"created\n")?;

        let mut start_fifo = File::options().read(true).open(start_fifo_path)?;
        let mut buf = String::new();
        start_fifo.read_to_string(&mut buf)?;

        self.container.reload()?;

        let mut child = process.spawn()?;
        self.container.state.status = Status::Running;
        self.container.save()?;

        let status = child.wait()?;
        self.container.state.status = Status::Stopped;
        self.container.save()?;

        Ok(status)
    }

    fn pass_pty_master(&self, console_socket: &str, master_fd: BorrowedFd) -> anyhow::Result<()> {
        let socket = UnixStream::connect(console_socket)?;
        let socket_fd = socket.as_raw_fd();

        let request_bytes = json!({
            "type": "terminal",
            "container": self.container.id,
        })
        .to_string()
        .into_bytes();
        let request = IoSlice::new(&request_bytes);

        let fds = [master_fd.as_raw_fd()];
        let cmsg = ControlMessage::ScmRights(&fds);

        nix::sys::socket::sendmsg::<()>(socket_fd, &[request], &[cmsg], MsgFlags::empty(), None)?;

        Ok(())
    }
}
