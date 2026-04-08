use serde::Serialize;
use std::io;
use std::process::Command;
use tracing::{info, warn};

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
pub fn run(json: bool) -> io::Result<bool> {
    let checks = vec![
        check_command("git", &["--version"], "git"),
        check_command("cargo", &["--version"], "Rust toolchain"),
        check_docker(),
        check_command("tmux", &["-V"], "tmux"),
        check_gh_auth(),
        check_docker_image("palette-base:latest"),
        check_docker_image("palette-member:latest"),
        check_docker_image("palette-leader:latest"),
    ];

    let all_ok = checks.iter().all(|c| c.ok);
    let report = DoctorReport { all_ok, checks };

    if json {
        let output = serde_json::to_string_pretty(&report)
            .map_err(io::Error::other)?;
        println!("{output}");
    } else {
        print_human_report(&report);
    }

    Ok(report.all_ok)
}

fn check_command(cmd: &str, args: &[&str], label: &str) -> CheckResult {
    match Command::new(cmd).args(args).output() {
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
        Err(e) => CheckResult {
            name: label.to_string(),
            ok: false,
            version: None,
            message: format!("{cmd}: {e}"),
        },
    }
}

fn check_docker() -> CheckResult {
    match Command::new("docker").args(["info"]).output() {
        Ok(output) if output.status.success() => {
            let version = Command::new("docker")
                .args(["--version"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
            CheckResult {
                name: "Docker".to_string(),
                ok: true,
                version,
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
        Err(e) => CheckResult {
            name: "Docker".to_string(),
            ok: false,
            version: None,
            message: format!("docker: {e}"),
        },
    }
}

fn check_gh_auth() -> CheckResult {
    match Command::new("gh").args(["auth", "status"]).output() {
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
        Err(e) => CheckResult {
            name: "GitHub CLI auth".to_string(),
            ok: false,
            version: None,
            message: format!("gh: {e}"),
        },
    }
}

fn check_docker_image(image: &str) -> CheckResult {
    match Command::new("docker")
        .args(["image", "inspect", image])
        .output()
    {
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
        Err(e) => CheckResult {
            name: format!("Docker image: {image}"),
            ok: false,
            version: None,
            message: format!("docker: {e}"),
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
