use std::{
    fs::File,
    io::{IoSlice, Read, Write},
    os::{fd::AsRawFd, unix::net::UnixStream},
    process::ExitStatus,
};

use log::error;
use nix::{
    libc,
    sched::{CloneCb, CloneFlags},
    sys::{
        socket::{ControlMessage, MsgFlags},
        stat::Mode,
    },
};
use oci_spec::runtime::Spec;
use serde_json::json;

use crate::{
    container::{Container, Status},
    process::Process,
};

#[derive(Debug, Default)]
pub struct Sandbox {
    container: Container,
    spec: Spec,
    console_socket: Option<String>,
    pipe_write: Option<File>,
}

impl Sandbox {
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
            pipe_write: Some(pipe_write),
        }
    }

    pub fn spawn(mut self) -> anyhow::Result<i32> {
        let callback: CloneCb = Box::new(|| match self.execute() {
            Ok(status) => status.code().unwrap().try_into().unwrap(),
            Err(err) => {
                error!("sandbox error: {}", err);
                1
            }
        });

        let mut stack = [0u8; 8192];

        let pid = unsafe { nix::sched::clone(callback, &mut stack, CloneFlags::empty(), None)? };

        Ok(pid.as_raw())
    }

    fn execute(&mut self) -> anyhow::Result<ExitStatus> {
        self.maybe_setup_pty()?;

        let mut process = Process::try_from(self.spec.clone())?.0;

        self.dispatch_created_event()?;
        self.wait_for_start_command()?;
        self.container.reload()?;

        let mut child = process.spawn()?;
        self.container.state.status = Status::Running;
        self.container.save()?;

        let status = child.wait()?;
        self.container.state.status = Status::Stopped;
        self.container.save()?;

        Ok(status)
    }

    fn maybe_setup_pty(&self) -> anyhow::Result<()> {
        let Some(console_socket) = &self.console_socket else {
            return Ok(());
        };

        let pty = nix::pty::openpty(None, None)?;

        nix::unistd::setsid()?;
        let ret = unsafe { libc::ioctl(pty.slave.as_raw_fd(), libc::TIOCSCTTY, 0) };
        assert!(ret == 0);

        nix::unistd::dup2(pty.slave.as_raw_fd(), 0)?;
        nix::unistd::dup2(pty.slave.as_raw_fd(), 1)?;
        nix::unistd::dup2(pty.slave.as_raw_fd(), 2)?;

        let socket = UnixStream::connect(console_socket)?;
        let socket_fd = socket.as_raw_fd();

        let request_bytes = json!({
            "type": "terminal",
            "container": self.container.id,
        })
        .to_string()
        .into_bytes();
        let request = IoSlice::new(&request_bytes);

        let fds = [pty.master.as_raw_fd()];
        let cmsg = ControlMessage::ScmRights(&fds);

        nix::sys::socket::sendmsg::<()>(socket_fd, &[request], &[cmsg], MsgFlags::empty(), None)?;
        Ok(())
    }

    fn dispatch_created_event(&mut self) -> anyhow::Result<()> {
        self.pipe_write.take().unwrap().write_all(b"created")?;
        Ok(())
    }

    fn wait_for_start_command(&self) -> anyhow::Result<()> {
        let start_fifo_path = self.container.runtime_dir().join("start");
        nix::unistd::mkfifo(&start_fifo_path, Mode::S_IRUSR | Mode::S_IWUSR)?;

        let mut start_fifo = File::options().read(true).open(start_fifo_path)?;

        let mut buf = String::new();
        start_fifo.read_to_string(&mut buf)?;

        Ok(())
    }
}
