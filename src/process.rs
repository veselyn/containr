use std::{
    fs::File,
    io::{IoSlice, Read},
    os::fd::AsRawFd,
    path::PathBuf,
    process::{Command, ExitStatus},
};

use anyhow::Context;
use nix::{
    libc,
    pty::OpenptyResult,
    sys::{
        socket::{ControlMessage, MsgFlags, SockFlag, SockType, UnixAddr},
        stat::Mode,
    },
};
use oci_spec::runtime::Spec;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct Process {
    pub container_id: String,
    pub spec: Spec,
    pub runtime_dir: PathBuf,
    pub console_socket: Option<String>,
}

impl Process {
    pub fn execute(self) -> anyhow::Result<ExitStatus> {
        let pty = self
            .console_socket
            .as_ref()
            .map(|console_socket| -> anyhow::Result<OpenptyResult> {
                let pty = nix::pty::openpty(None, None)?;

                let socket = nix::sys::socket::socket(
                    nix::sys::socket::AddressFamily::Unix,
                    SockType::Stream,
                    SockFlag::empty(),
                    None,
                )?;

                let socket_fd = socket.as_raw_fd();

                let unix_addr = UnixAddr::new(console_socket.as_str())?;

                nix::sys::socket::connect(socket_fd, &unix_addr)?;

                let request_bytes = json!({
                    "type": "terminal",
                    "container": self.container_id,
                })
                .to_string()
                .into_bytes();
                let request = IoSlice::new(&request_bytes);

                let fds = [pty.master.as_raw_fd()];
                let cmsg = ControlMessage::ScmRights(&fds);

                nix::sys::socket::sendmsg::<()>(
                    socket_fd,
                    &[request],
                    &[cmsg],
                    MsgFlags::empty(),
                    None,
                )?;

                Ok(pty)
            })
            .transpose()?;

        let start_fifo_path = self.runtime_dir.join("start");
        nix::unistd::mkfifo(&start_fifo_path, Mode::S_IRUSR | Mode::S_IWUSR)?;

        let mut start_fifo = File::options().read(true).open(start_fifo_path)?;

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

        let mut buf = String::new();
        start_fifo.read_to_string(&mut buf)?;

        let status = process.status()?;

        Ok(status)
    }
}
