use super::route;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayInfo {
    pub gateways: Vec<Gateway>,
    pub has_multiple: bool,
    pub warning: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Gateway {
    pub ip: String,
    pub interface: String,
    pub metric: u32,
    pub is_primary: bool,
    pub latency_ms: Option<f32>,
    pub reachable: bool,
}

pub fn get_gateway_info() -> Result<GatewayInfo, String> {
    let gateways_raw = get_all_gateways()?;

    let mut gateways: Vec<Gateway> = gateways_raw
        .iter()
        .enumerate()
        .map(|(i, (ip, iface, metric))| {
            let ping_result = route::ping_host(ip, 3).ok();
            let latency_ms = ping_result.as_ref().map(|r| r.avg_ms);
            let reachable = ping_result.map_or(false, |r| r.packets_received > 0);

            Gateway {
                ip: ip.clone(),
                interface: iface.clone(),
                metric: *metric,
                is_primary: i == 0,
                latency_ms,
                reachable,
            }
        })
        .collect();

    let has_multiple = gateways.len() > 1;

    if has_multiple {
        gateways.sort_by_key(|g| g.metric);
        if let Some(first) = gateways.first_mut() {
            first.is_primary = true;
        }
    }

    let warning = if has_multiple {
        let primary = gateways
            .first()
            .map(|g| format!("{} ({})", g.interface, g.ip))
            .unwrap_or_default();
        Some(format!(
            "Gateway duplo detectado. Verifique se o roteamento assimétrico \
             está causando instabilidade. Interface primária: {}",
            primary
        ))
    } else {
        None
    };

    Ok(GatewayInfo {
        gateways,
        has_multiple,
        warning,
    })
}

fn get_all_gateways() -> Result<Vec<(String, String, u32)>, String> {
    if cfg!(target_os = "windows") {
        return get_gateways_windows();
    }

    let output = std::process::Command::new("ip")
        .args(["route", "show"])
        .output()
        .map_err(|e| format!("Falha ao executar ip route: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gateways = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("default") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                let gw = parts[2].to_string();
                let iface = parts[4].to_string();
                let metric = parts
                    .iter()
                    .position(|p| p == &"metric")
                    .and_then(|idx| parts.get(idx + 1))
                    .and_then(|m| m.parse::<u32>().ok())
                    .unwrap_or(100);
                gateways.push((gw, iface, metric));
            }
        }
    }

    if gateways.is_empty() {
        Err("Nenhuma rota padrão encontrada".to_string())
    } else {
        Ok(gateways)
    }
}

fn get_gateways_windows() -> Result<Vec<(String, String, u32)>, String> {
    let output = std::process::Command::new("netstat")
        .args(["-rn"])
        .output()
        .map_err(|e| format!("Falha ao executar netstat: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gateways = Vec::new();
    let mut metric = 1u32;

    for line in stdout.lines() {
        if line.contains("0.0.0.0") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 && parts[0] == "0.0.0.0" && parts[1] == "0.0.0.0" {
                let gw = parts[2].to_string();
                let iface = parts[4].to_string();
                gateways.push((gw, iface, metric));
                metric += 100;
            }
        }
    }

    if gateways.is_empty() {
        Err("Nenhum gateway padrão encontrado no Windows".to_string())
    } else {
        Ok(gateways)
    }
}
