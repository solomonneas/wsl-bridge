use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::BTreeSet;
use tokio::process::Command;

pub async fn detect_ports() -> (BTreeSet<u16>, BTreeSet<u16>) {
    let pm2_ports = detect_pm2_ports().await.unwrap_or_else(|err| {
        tracing::debug!(error = %err, "pm2 detection failed");
        BTreeSet::new()
    });

    let caddy_ports = detect_caddy_ports().await.unwrap_or_else(|err| {
        tracing::debug!(error = %err, "caddy detection failed");
        BTreeSet::new()
    });

    (pm2_ports, caddy_ports)
}

async fn detect_pm2_ports() -> Result<BTreeSet<u16>> {
    let output = Command::new("pm2")
        .arg("jlist")
        .output()
        .await
        .context("failed to execute pm2 jlist")?;

    if !output.status.success() {
        anyhow::bail!("pm2 jlist exited with {}", output.status);
    }

    let value: Value = serde_json::from_slice(&output.stdout).context("invalid pm2 json")?;
    let mut ports = BTreeSet::new();
    collect_ports_from_json(&value, &mut ports);
    Ok(ports)
}

async fn detect_caddy_ports() -> Result<BTreeSet<u16>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .context("failed to build reqwest client")?;

    let value: Value = client
        .get("http://localhost:2019/config/")
        .send()
        .await
        .context("failed requesting caddy config")?
        .error_for_status()
        .context("caddy config returned error status")?
        .json()
        .await
        .context("invalid caddy config json")?;

    let mut ports = BTreeSet::new();
    collect_ports_from_json(&value, &mut ports);
    Ok(ports)
}

fn collect_ports_from_json(value: &Value, out: &mut BTreeSet<u16>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if k.eq_ignore_ascii_case("port") || k.eq_ignore_ascii_case("listen_port") {
                    if let Some(p) = v.as_u64().and_then(to_valid_port) {
                        out.insert(p);
                    }
                }

                if k.eq_ignore_ascii_case("listen") || k.eq_ignore_ascii_case("address") {
                    if let Some(s) = v.as_str() {
                        for p in extract_ports_from_string(s) {
                            out.insert(p);
                        }
                    }
                }

                collect_ports_from_json(v, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_ports_from_json(item, out);
            }
        }
        Value::String(s) => {
            for p in extract_ports_from_string(s) {
                out.insert(p);
            }
        }
        _ => {}
    }
}

fn to_valid_port(v: u64) -> Option<u16> {
    let p = u16::try_from(v).ok()?;
    if p == 0 {
        return None;
    }
    Some(p)
}

fn extract_ports_from_string(input: &str) -> Vec<u16> {
    let mut out = Vec::new();

    if let Some(stripped) = input.strip_prefix(':') {
        if let Ok(p) = stripped.parse::<u16>() {
            if p != 0 {
                out.push(p);
                return out;
            }
        }
    }

    if let Some(idx) = input.rfind(':') {
        let suffix = &input[idx + 1..];
        let suffix = suffix.trim_end_matches('/');
        if let Ok(p) = suffix.parse::<u16>() {
            if p != 0 {
                out.push(p);
            }
        }
    }

    out
}
