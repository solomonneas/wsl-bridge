mod config;
mod detector;
mod windows;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::BTreeSet;
use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Parser, Debug)]
#[command(name = "wsl-port")]
#[command(about = "WSL to Windows portproxy auto-forwarder", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show current IP, configured ports, and netsh mappings
    Status,
    /// Add a port to the manual config and sync immediately
    Add { port: u16 },
    /// Remove a port from the manual config and sync immediately
    Remove { port: u16 },
    /// Force immediate re-sync of netsh rules
    Sync,
    /// Run daemon loop and refresh rules on IP/config changes
    Daemon,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Status => cmd_status().await,
        Commands::Add { port } => cmd_add(port).await,
        Commands::Remove { port } => cmd_remove(port).await,
        Commands::Sync => cmd_sync().await,
        Commands::Daemon => cmd_daemon().await,
    }
}

async fn cmd_status() -> Result<()> {
    let path = config::config_path()?;
    let mut cfg = config::load_or_default(&path)?;

    let (pm2_ports, caddy_ports) = detector::detect_ports().await;
    cfg.set_detected_ports(pm2_ports, caddy_ports);
    config::save(&path, &cfg)?;

    let current_ip = get_wsl_ip().await?;
    let all_ports = cfg.all_ports();
    let rules = windows::show_portproxy().await.unwrap_or_else(|err| {
        format!("Could not fetch netsh mappings: {err}")
    });

    println!("WSL IP: {current_ip}");
    println!("Config file: {}", path.display());
    println!("Manual ports: {:?}", cfg.manual_ports);
    println!("PM2 ports: {:?}", cfg.pm2_ports);
    println!("Caddy ports: {:?}", cfg.caddy_ports);
    println!("All forwarded ports: {:?}", all_ports);
    println!("\nCurrent netsh portproxy mappings:\n{rules}");

    Ok(())
}

async fn cmd_add(port: u16) -> Result<()> {
    ensure_valid_port(port)?;

    let path = config::config_path()?;
    let mut cfg = config::load_or_default(&path)?;

    let inserted = cfg.add_manual_port(port);
    let (pm2_ports, caddy_ports) = detector::detect_ports().await;
    cfg.set_detected_ports(pm2_ports, caddy_ports);
    config::save(&path, &cfg)?;

    sync_current_config(&cfg).await?;

    if inserted {
        println!("Added port {port} and synced rules.");
    } else {
        println!("Port {port} already present; synced rules anyway.");
    }

    Ok(())
}

async fn cmd_remove(port: u16) -> Result<()> {
    ensure_valid_port(port)?;

    let path = config::config_path()?;
    let mut cfg = config::load_or_default(&path)?;

    let removed = cfg.remove_manual_port(port);
    let (pm2_ports, caddy_ports) = detector::detect_ports().await;
    cfg.set_detected_ports(pm2_ports, caddy_ports);
    config::save(&path, &cfg)?;

    sync_current_config(&cfg).await?;

    if removed {
        println!("Removed port {port} and synced rules.");
    } else {
        println!("Port {port} was not in manual config; synced rules anyway.");
    }

    Ok(())
}

async fn cmd_sync() -> Result<()> {
    let path = config::config_path()?;
    let mut cfg = config::load_or_default(&path)?;
    let (pm2_ports, caddy_ports) = detector::detect_ports().await;
    cfg.set_detected_ports(pm2_ports, caddy_ports);
    config::save(&path, &cfg)?;

    sync_current_config(&cfg).await?;
    println!("Sync complete.");
    Ok(())
}

async fn cmd_daemon() -> Result<()> {
    tracing::info!("starting daemon; poll interval = 5s");

    let path = config::config_path()?;
    let mut last_ip: Option<Ipv4Addr> = None;
    let mut last_ports: BTreeSet<u16> = BTreeSet::new();

    loop {
        let mut cfg = config::load_or_default(&path)?;
        let (pm2_ports, caddy_ports) = detector::detect_ports().await;
        cfg.set_detected_ports(pm2_ports, caddy_ports);
        config::save(&path, &cfg)?;

        let ip = get_wsl_ip().await?;
        let ports = cfg.all_ports();

        let changed = last_ip != Some(ip) || last_ports != ports;
        if changed {
            let sorted_ports: Vec<u16> = ports.iter().copied().collect();
            tracing::info!(ip = %ip, ports = ?sorted_ports, "change detected; syncing portproxy rules");
            windows::apply_portproxy_rules(ip, &sorted_ports).await?;
            last_ip = Some(ip);
            last_ports = ports;
        }

        sleep(Duration::from_secs(5)).await;
    }
}

async fn sync_current_config(cfg: &config::PortsConfig) -> Result<()> {
    let ip = get_wsl_ip().await?;
    let ports: Vec<u16> = cfg.all_ports().into_iter().collect();
    windows::apply_portproxy_rules(ip, &ports).await?;
    Ok(())
}

fn ensure_valid_port(port: u16) -> Result<()> {
    if port == 0 {
        anyhow::bail!("port 0 is invalid")
    }
    Ok(())
}

async fn get_wsl_ip() -> Result<Ipv4Addr> {
    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg("hostname -I")
        .output()
        .await
        .context("failed to run hostname -I")?;

    if !output.status.success() {
        anyhow::bail!("hostname -I failed with {}", output.status);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let candidate = stdout
        .split_whitespace()
        .find_map(|token| token.parse::<Ipv4Addr>().ok())
        .context("could not parse IPv4 from hostname -I output")?;

    Ok(candidate)
}
