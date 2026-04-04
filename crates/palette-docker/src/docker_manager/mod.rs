mod claude_exec_command;
mod copy_dir_to_container;
mod copy_file_to_container;
pub(crate) mod create_container;
mod is_claude_running;
mod is_container_running;
mod list_managed_containers;
mod remove_container;
mod start_container;
mod stop_container;
mod write_settings;

pub use is_container_running::is_container_running;

use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackNetworkMode {
    Auto,
    Host,
    Bridge,
}

pub struct DockerManager {
    worker_callback_url: String,
    callback_network_mode: CallbackNetworkMode,
}

impl DockerManager {
    pub fn new(worker_callback_url: String, callback_network_mode: CallbackNetworkMode) -> Self {
        let resolved = match callback_network_mode {
            CallbackNetworkMode::Auto => {
                if cfg!(target_os = "linux") {
                    CallbackNetworkMode::Host
                } else {
                    CallbackNetworkMode::Bridge
                }
            }
            mode => mode,
        };

        let worker_callback_url = if resolved == CallbackNetworkMode::Bridge {
            normalize_bridge_callback_url(&worker_callback_url)
        } else {
            worker_callback_url
        };

        Self {
            worker_callback_url,
            callback_network_mode: resolved,
        }
    }
}

fn normalize_bridge_callback_url(url: &str) -> String {
    const MAP: [(&str, &str); 4] = [
        ("http://127.0.0.1", "http://host.docker.internal"),
        ("https://127.0.0.1", "https://host.docker.internal"),
        ("http://localhost", "http://host.docker.internal"),
        ("https://localhost", "https://host.docker.internal"),
    ];

    for (from, to) in MAP {
        if url.starts_with(from) {
            return url.replacen(from, to, 1);
        }
    }
    url.to_string()
}

pub(super) fn run_docker<I, S>(args: I) -> crate::Result<std::process::Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Ok(Command::new("docker").args(args).output()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_mode_rewrites_loopback_host() {
        let mgr = DockerManager::new(
            "http://127.0.0.1:7100".to_string(),
            CallbackNetworkMode::Bridge,
        );
        assert_eq!(
            mgr.worker_callback_url,
            "http://host.docker.internal:7100".to_string()
        );
    }

    #[test]
    fn bridge_mode_keeps_non_loopback_host() {
        let mgr = DockerManager::new(
            "http://example.test:7100".to_string(),
            CallbackNetworkMode::Bridge,
        );
        assert_eq!(mgr.worker_callback_url, "http://example.test:7100");
    }
}
