use std::{
    fs::File,
    io::{IoSlice, Read, Write},
    os::{
        fd::{AsFd, AsRawFd},
        unix::{fs::OpenOptionsExt, net::UnixStream},
    },
    process::ExitStatus,
    time::Duration,
};

use log::error;
use nix::{
    libc,
    mount::{MntFlags, MsFlags},
    poll::{PollFd, PollFlags, PollTimeout},
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

#[derive(Debug)]
pub struct Sandbox<'a> {
    container: &'a mut Container,
    spec: Spec,
    console_socket: Option<String>,
    created_event_pipe_writer: Option<File>,
    start_command_fifo_reader: Option<File>,
}

impl<'a> Sandbox<'a> {
    pub fn new(
        container: &'a mut Container,
        spec: Spec,
        console_socket: Option<String>,
        created_event_pipe_writer: File,
    ) -> anyhow::Result<Self> {
        let start_command_fifo_path = container.runtime_dir().join("start");
        nix::unistd::mkfifo(&start_command_fifo_path, Mode::S_IRUSR | Mode::S_IWUSR)?;

        let start_command_fifo_reader = File::options()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(start_command_fifo_path)?;

        Ok(Self {
            container,
            spec,
            console_socket,
            created_event_pipe_writer: Some(created_event_pipe_writer),
            start_command_fifo_reader: Some(start_command_fifo_reader),
        })
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

        let pid =
            unsafe { nix::sched::clone(callback, &mut stack, CloneFlags::CLONE_NEWNS, None)? };

        Ok(pid.as_raw())
    }

    fn execute(&mut self) -> anyhow::Result<ExitStatus> {
        self.maybe_setup_pty()?;
        self.pivot_root()?;

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

    fn pivot_root(&self) -> anyhow::Result<()> {
        let root = self.spec.root().as_ref().unwrap().path();

        nix::unistd::chdir(root)?;
        nix::unistd::pivot_root(".", ".")?; // Stacks mount points

        nix::mount::mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_SLAVE | MsFlags::MS_REC,
            None::<&str>,
        )?;

        nix::mount::umount2("/", MntFlags::MNT_DETACH)?;

        Ok(())
    }

    fn dispatch_created_event(&mut self) -> anyhow::Result<()> {
        self.created_event_pipe_writer
            .take()
            .unwrap()
            .write_all(b"created")?;

        Ok(())
    }

    fn wait_for_start_command(&mut self) -> anyhow::Result<()> {
        let mut start_command_fifo_reader = self.start_command_fifo_reader.take().unwrap();

        nix::poll::poll(
            &mut [PollFd::new(
                start_command_fifo_reader.as_fd(),
                PollFlags::POLLIN,
            )],
            PollTimeout::try_from(Duration::from_secs(5))?,
        )?;

        let mut buf = String::new();
        start_command_fifo_reader.read_to_string(&mut buf)?;
        assert!(buf == "start");

        Ok(())
    }
}
