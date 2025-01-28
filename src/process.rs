use std::process::Command;

use anyhow::Context;
use oci_spec::runtime::Spec;

#[derive(Debug)]
pub struct Process(pub Command);

impl TryFrom<Spec> for Process {
    type Error = anyhow::Error;

    fn try_from(spec: Spec) -> Result<Self, Self::Error> {
        let spec_process = spec.process().as_ref().context("no process in spec")?;

        let mut args = spec_process
            .args()
            .as_ref()
            .context("no process args in spec")?
            .iter();

        let mut command = Command::new(args.next().context("process args are empty")?);
        command.args(args);
        command.env_clear();
        command.envs(
            spec_process
                .env()
                .clone()
                .unwrap_or_default()
                .iter()
                .map(|e| e.split_once("=").unwrap()),
        );
        command.current_dir(spec_process.cwd());

        Ok(Self(command))
    }
}
