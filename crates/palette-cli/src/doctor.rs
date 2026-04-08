use serde::Serialize;
use std::io;
use std::time::Duration;
use tokio::process::Command;
use tracing::{info, warn};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Serialize)]
struct CheckResult {
    name: String,
    ok: bool,
    version: Option<String>,
    message: String,
}

#[derive(Serialize)]
struct DoctorReport {
    all_ok: bool,
    checks: Vec<CheckResult>,
}

/// Runs all prerequisite checks and prints results.
/// Returns Ok(true) if all checks passed, Ok(false) if any failed.
pub async fn run(json: bool) -> io::Result<bool> {
    let checks = vec![
        check_command("git", &["--version"], "git").await,
        check_command("cargo", &["--version"], "Rust toolchain").await,
        check_docker().await,
        check_command("tmux", &["-V"], "tmux").await,
        check_gh_auth().await,
        check_docker_image("palette-base:latest").await,
        check_docker_image("palette-member:latest").await,
        check_docker_image("palette-leader:latest").await,
    ];

    let all_ok = checks.iter().all(|c| c.ok);
    let report = DoctorReport { all_ok, checks };

    if json {
        let output = serde_json::to_string_pretty(&report).map_err(io::Error::other)?;
        println!("{output}");
    } else {
        print_human_report(&report);
    }

    Ok(report.all_ok)
}

async fn run_command(cmd: &str, args: &[&str]) -> Result<std::process::Output, String> {
    let child = Command::new(cmd).args(args).output();

    match tokio::time::timeout(COMMAND_TIMEOUT, child).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(format!("{cmd}: {e}")),
        Err(_) => Err(format!(
            "{cmd}: timed out after {}s",
            COMMAND_TIMEOUT.as_secs()
        )),
    }
}

async fn check_command(cmd: &str, args: &[&str], label: &str) -> CheckResult {
    match run_command(cmd, args).await {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            CheckResult {
                name: label.to_string(),
                ok: true,
                version: Some(stdout),
                message: format!("{label} is available"),
            }
        }
        Ok(output) => CheckResult {
            name: label.to_string(),
            ok: false,
            version: None,
            message: format!("{cmd} exited with {}", output.status.code().unwrap_or(-1)),
        },
        Err(msg) => CheckResult {
            name: label.to_string(),
            ok: false,
            version: None,
            message: msg,
        },
    }
}

async fn check_docker() -> CheckResult {
    // Use `docker version` instead of `docker info` — it only checks
    // client/server version and is far less likely to hang on macOS
    // where `docker info` can stall collecting system details.
    match run_command("docker", &["version", "--format", "{{.Server.Version}}"]).await {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            CheckResult {
                name: "Docker".to_string(),
                ok: true,
                version: Some(version),
                message: "Docker daemon is running".to_string(),
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            CheckResult {
                name: "Docker".to_string(),
                ok: false,
                version: None,
                message: if stderr.is_empty() {
                    "Docker daemon is not running".to_string()
                } else {
                    format!("Docker daemon is not running: {stderr}")
                },
            }
        }
        Err(msg) => CheckResult {
            name: "Docker".to_string(),
            ok: false,
            version: None,
            message: msg,
        },
    }
}

async fn check_gh_auth() -> CheckResult {
    match run_command("gh", &["auth", "status"]).await {
        Ok(output) if output.status.success() => CheckResult {
            name: "GitHub CLI auth".to_string(),
            ok: true,
            version: None,
            message: "Authenticated".to_string(),
        },
        Ok(_) => CheckResult {
            name: "GitHub CLI auth".to_string(),
            ok: false,
            version: None,
            message: "Not authenticated — run `gh auth login`".to_string(),
        },
        Err(msg) => CheckResult {
            name: "GitHub CLI auth".to_string(),
            ok: false,
            version: None,
            message: msg,
        },
    }
}

async fn check_docker_image(image: &str) -> CheckResult {
    match run_command("docker", &["image", "inspect", image]).await {
        Ok(output) if output.status.success() => CheckResult {
            name: format!("Docker image: {image}"),
            ok: true,
            version: None,
            message: "Found".to_string(),
        },
        Ok(_) => CheckResult {
            name: format!("Docker image: {image}"),
            ok: false,
            version: None,
            message: "Not found — run `scripts/build-images.sh`".to_string(),
        },
        Err(msg) => CheckResult {
            name: format!("Docker image: {image}"),
            ok: false,
            version: None,
            message: msg,
        },
    }
}

fn print_human_report(report: &DoctorReport) {
    for check in &report.checks {
        if check.ok {
            info!(name = %check.name, "{}", check.message);
        } else {
            warn!(name = %check.name, "{}", check.message);
        }
    }
    if report.all_ok {
        info!("All checks passed.");
    } else {
        let failed = report.checks.iter().filter(|c| !c.ok).count();
        warn!("{failed} check(s) failed.");
    }
}
