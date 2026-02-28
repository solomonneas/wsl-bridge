use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PortsConfig {
    #[serde(default)]
    pub manual_ports: BTreeSet<u16>,
    #[serde(default)]
    pub pm2_ports: BTreeSet<u16>,
    #[serde(default)]
    pub caddy_ports: BTreeSet<u16>,
}

impl PortsConfig {
    pub fn all_ports(&self) -> BTreeSet<u16> {
        self.manual_ports
            .iter()
            .chain(self.pm2_ports.iter())
            .chain(self.caddy_ports.iter())
            .copied()
            .collect()
    }

    pub fn add_manual_port(&mut self, port: u16) -> bool {
        self.manual_ports.insert(port)
    }

    pub fn remove_manual_port(&mut self, port: u16) -> bool {
        self.manual_ports.remove(&port)
    }

    pub fn set_detected_ports(&mut self, pm2_ports: BTreeSet<u16>, caddy_ports: BTreeSet<u16>) {
        self.pm2_ports = pm2_ports;
        self.caddy_ports = caddy_ports;
    }
}

pub fn config_dir() -> Result<PathBuf> {
    let base = dirs::config_dir().context("could not resolve config directory")?;
    Ok(base.join("wsl-port-forwarder"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("ports.toml"))
}

pub fn load_or_default(path: &Path) -> Result<PortsConfig> {
    if !path.exists() {
        return Ok(PortsConfig::default());
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed reading config from {}", path.display()))?;
    let cfg: PortsConfig = toml::from_str(&raw)
        .with_context(|| format!("failed parsing toml from {}", path.display()))?;
    Ok(cfg)
}

pub fn save(path: &Path, cfg: &PortsConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed creating config dir {}", parent.display()))?;
    }

    let raw = toml::to_string_pretty(cfg).context("failed serializing config")?;
    fs::write(path, raw).with_context(|| format!("failed writing config {}", path.display()))?;
    Ok(())
}
