use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct InterfaceInfo {
    pub name: String,
    pub ip: String,
    pub mac: String,
    pub is_up: bool,
}

pub fn get_network_interfaces() -> Result<Vec<InterfaceInfo>, String> {
    if cfg!(target_os = "windows") {
        return get_interfaces_windows();
    }

    let output = std::process::Command::new("ip")
        .args(["-j", "addr", "show"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            parse_ip_json(&stdout)
        }
        _ => {
            let output = std::process::Command::new("ip")
                .args(["addr", "show"])
                .output()
                .map_err(|e| format!("Falha ao executar ip addr: {}", e))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            parse_ip_text(&stdout)
        }
    }
}

fn parse_ip_json(output: &str) -> Result<Vec<InterfaceInfo>, String> {
    let parsed: Vec<serde_json::Value> = serde_json::from_str(output)
        .map_err(|e| format!("Erro ao parsear JSON do ip: {}", e))?;

    let mut interfaces = Vec::new();

    for iface in parsed {
        let name = iface["ifname"].as_str().unwrap_or("unknown").to_string();
        let is_up = iface["flags"]
            .as_array()
            .map(|f| f.iter().any(|v| v.as_str() == Some("UP")))
            .unwrap_or(false);
        let mac = iface["address"].as_str().unwrap_or("").to_string();

        let ip = if let Some(addr_info) = iface["addr_info"].as_array() {
            addr_info
                .iter()
                .find(|a| a["family"].as_str() == Some("inet"))
                .and_then(|a| a["local"].as_str())
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        };

        if !name.starts_with("lo") {
            interfaces.push(InterfaceInfo { name, ip, mac, is_up });
        }
    }

    if interfaces.is_empty() {
        Err("Nenhuma interface de rede encontrada".to_string())
    } else {
        Ok(interfaces)
    }
}

fn parse_ip_text(output: &str) -> Result<Vec<InterfaceInfo>, String> {
    let mut interfaces = Vec::new();
    let mut current_name = String::new();
    let mut current_mac = String::new();
    let mut current_ip = String::new();
    let mut current_up = false;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("link/ether") || trimmed.starts_with("link/loopback") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                current_mac = parts[1].to_string();
            }
        }
        if trimmed.starts_with("inet ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                current_ip = parts[1].split('/').next().unwrap_or("").to_string();
            }
        }
        if trimmed.starts_with("inet6 ") {
            continue;
        }
        if trimmed.ends_with(':') && !trimmed.starts_with("inet") && !trimmed.starts_with("link") {
            if !current_name.is_empty() && !current_name.starts_with("lo") {
                interfaces.push(InterfaceInfo {
                    name: current_name.clone(),
                    ip: current_ip.clone(),
                    mac: current_mac.clone(),
                    is_up: current_up,
                });
            }
            current_name = trimmed.trim_end_matches(':').to_string();
            current_up = trimmed.contains("UP");
            current_mac.clear();
            current_ip.clear();
        }
    }

    if !current_name.is_empty() && !current_name.starts_with("lo") {
        interfaces.push(InterfaceInfo {
            name: current_name,
            ip: current_ip,
            mac: current_mac,
            is_up: current_up,
        });
    }

    if interfaces.is_empty() {
        Err("Nenhuma interface de rede encontrada".to_string())
    } else {
        Ok(interfaces)
    }
}

fn get_interfaces_windows() -> Result<Vec<InterfaceInfo>, String> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-NetAdapter | Select-Object Name, InterfaceDescription, MacAddress, Status | ConvertTo-Json",
        ])
        .output()
        .map_err(|e| format!("Falha ao executar Get-NetAdapter: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
        let mut interfaces = Vec::new();
        for iface in parsed {
            let name = iface["Name"].as_str().unwrap_or("unknown").to_string();
            let mac = iface["MacAddress"].as_str().unwrap_or("").to_string();
            let is_up = iface["Status"].as_str() == Some("Up");

            let ip = get_windows_ip(&name).unwrap_or_default();

            interfaces.push(InterfaceInfo { name, ip, mac, is_up });
        }
        Ok(interfaces)
    } else {
        Err("Nenhuma interface encontrada no Windows".to_string())
    }
}

fn get_windows_ip(iface_name: &str) -> Option<String> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Get-NetIPAddress -InterfaceAlias '{}' -AddressFamily IPv4 | Select-Object -ExpandProperty IPAddress",
                iface_name.replace('\'', "''")
            ),
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let ip = String::from_utf8_lossy(&output.stdout);
        Some(ip.trim().to_string())
    } else {
        None
    }
}

pub fn get_public_ip_address() -> Result<String, String> {
    let config = ureq::config::Config::builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let resp = agent
        .get("https://api.ipify.org")
        .call()
        .map_err(|e| format!("Falha ao consultar IP público: {}", e))?;

    let mut body = resp.into_body();
    let ip = body
        .read_to_string()
        .map_err(|e| format!("Erro ao ler resposta: {}", e))?;

    Ok(ip.trim().to_string())
}
