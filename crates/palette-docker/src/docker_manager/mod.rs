mod claude_exec_command;
mod copy_dir_to_container;
mod copy_file_to_container;
mod create_container;
mod remove_container;
mod start_container;
mod stop_container;
mod write_settings;

use std::process::Command;

pub struct DockerManager {
    palette_url: String,
}

impl DockerManager {
    pub fn new(palette_url: String) -> Self {
        Self { palette_url }
    }
}

pub(super) fn run_docker<I, S>(args: I) -> crate::Result<std::process::Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Ok(Command::new("docker").args(args).output()?)
}
