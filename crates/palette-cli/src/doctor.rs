use serde::Serialize;
use std::process::Command;

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

/// Returns true if all checks passed, false otherwise.
pub fn run(json: bool) -> bool {
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
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        print_human_report(&report);
    }

    report.all_ok
}

fn check_command(cmd: &str, args: &[&str], label: &str) -> CheckResult {
    match Command::new(cmd).args(args).output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            CheckResult {
                name: label.to_string(),
                ok: true,
                version: Some(stdout.clone()),
                message: format!("{label} is available"),
            }
        }
        Ok(output) => CheckResult {
            name: label.to_string(),
            ok: false,
            version: None,
            message: format!("{cmd} exited with {}", output.status.code().unwrap_or(-1)),
        },
        Err(_) => CheckResult {
            name: label.to_string(),
            ok: false,
            version: None,
            message: format!("{cmd} not found"),
        },
    }
}

fn check_docker() -> CheckResult {
    match Command::new("docker").args(["info"]).output() {
        Ok(output) if output.status.success() => CheckResult {
            name: "Docker".to_string(),
            ok: true,
            version: extract_docker_version(),
            message: "Docker daemon is running".to_string(),
        },
        Ok(_) => CheckResult {
            name: "Docker".to_string(),
            ok: false,
            version: None,
            message: "Docker daemon is not running".to_string(),
        },
        Err(_) => CheckResult {
            name: "Docker".to_string(),
            ok: false,
            version: None,
            message: "docker not found".to_string(),
        },
    }
}

fn extract_docker_version() -> Option<String> {
    Command::new("docker")
        .args(["--version"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
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
        Err(_) => CheckResult {
            name: "GitHub CLI auth".to_string(),
            ok: false,
            version: None,
            message: "gh not found — install GitHub CLI".to_string(),
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
        _ => CheckResult {
            name: format!("Docker image: {image}"),
            ok: false,
            version: None,
            message: "Not found — run `scripts/build-images.sh`".to_string(),
        },
    }
}

fn print_human_report(report: &DoctorReport) {
    for check in &report.checks {
        let icon = if check.ok { "ok" } else { "FAIL" };
        println!("[{icon:>4}] {}: {}", check.name, check.message);
    }
    println!();
    if report.all_ok {
        println!("All checks passed.");
    } else {
        let failed = report.checks.iter().filter(|c| !c.ok).count();
        println!("{failed} check(s) failed.");
    }
}
