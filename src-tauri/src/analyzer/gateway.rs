use serde::Serialize;
use super::route;

#[derive(Debug, Serialize)]
pub struct GatewayInfo {
    pub ip: String,
    pub interface: String,
    pub latency_ms: Option<f32>,
    pub quality: String,
}

pub fn get_gateway_info() -> Result<GatewayInfo, String> {
    let (gw_ip, gw_iface) = get_default_gateway()?;

    let latency = route::ping_host(&gw_ip, 5)
        .ok()
        .map(|r| r.avg_ms);

    let quality = latency
        .map(|ms| route::classify_latency(ms))
        .unwrap_or("Desconhecida")
        .to_string();

    Ok(GatewayInfo {
        ip: gw_ip,
        interface: gw_iface,
        latency_ms: latency,
        quality,
    })
}

fn get_default_gateway() -> Result<(String, String), String> {
    if cfg!(target_os = "windows") {
        return get_gateway_windows();
    }

    let output = std::process::Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .map_err(|e| format!("Falha ao executar ip route: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Some(line) = stdout.lines().next() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 && parts[0] == "default" {
            let gw = parts[2].to_string();
            let iface = parts[4].to_string();
            return Ok((gw, iface));
        }
        Err("Nenhuma rota padrão encontrada".to_string())
    } else {
        Err("Nenhuma rota padrão encontrada".to_string())
    }
}

fn get_gateway_windows() -> Result<(String, String), String> {
    let output = std::process::Command::new("netstat")
        .args(["-rn"])
        .output()
        .map_err(|e| format!("Falha ao executar netstat: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if line.contains("0.0.0.0") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 && parts[0] == "0.0.0.0" && parts[1] == "0.0.0.0" {
                return Ok((parts[2].to_string(), parts[4].to_string()));
            }
        }
    }

    Err("Nenhum gateway padrão encontrado no Windows".to_string())
}
