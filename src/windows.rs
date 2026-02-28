use anyhow::{Context, Result};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use tokio::process::Command;

fn find_powershell() -> PathBuf {
    let candidates = [
        "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe",
        "/mnt/c/WINDOWS/System32/WindowsPowerShell/v1.0/powershell.exe",
    ];
    
    for path in &candidates {
        if std::fs::metadata(path).is_ok() {
            return PathBuf::from(path);
        }
    }
    
    PathBuf::from("powershell.exe")
}

pub async fn apply_portproxy_rules(wsl_ip: Ipv4Addr, ports: &[u16]) -> Result<()> {
    let ps = find_powershell();
    
    for &port in ports {
        let delete_cmd = format!(
            "netsh interface portproxy delete v4tov4 listenport={} listenaddress=0.0.0.0",
            port
        );
        // Ignore delete errors (rule might not exist)
        let _ = run_powershell(&ps, &delete_cmd).await;

        let add_cmd = format!(
            "netsh interface portproxy add v4tov4 listenport={} listenaddress=0.0.0.0 connectport={} connectaddress={}",
            port, port, wsl_ip
        );
        run_powershell(&ps, &add_cmd).await?;
    }

    Ok(())
}

pub async fn show_portproxy() -> Result<String> {
    let ps = find_powershell();
    run_powershell_capture(&ps, "netsh interface portproxy show v4tov4").await
}

async fn run_powershell(powershell_path: &PathBuf, command: &str) -> Result<()> {
    let output = Command::new(powershell_path)
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(command)
        .output()
        .await
        .with_context(|| format!("failed to launch powershell for command: {command}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Delete fails if rule doesn't exist - that's ok
    if command.contains("portproxy delete") {
        return Ok(());
    }

    anyhow::bail!(
        "powershell command failed ({}): {}",
        output.status,
        stderr.trim()
    )
}

async fn run_powershell_capture(powershell_path: &PathBuf, command: &str) -> Result<String> {
    let output = Command::new(powershell_path)
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(command)
        .output()
        .await
        .with_context(|| format!("failed to launch powershell for command: {command}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "powershell command failed ({}): {}",
            output.status,
            stderr.trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
