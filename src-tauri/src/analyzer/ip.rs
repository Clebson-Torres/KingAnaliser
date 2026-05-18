use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct InterfaceInfo {
    pub name: String,
    pub ip: String,
    pub mac: String,
    pub is_up: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IpInfo {
    pub ipv4: String,
    pub ipv6: Option<String>,
    pub hostname: Option<String>,
    pub country: String,
    pub country_code: String,
    pub region: String,
    pub city: String,
    pub isp: String,
    pub org: String,
    pub asn: String,
    pub as_name: String,
    pub timezone: String,
    pub is_proxy: bool,
    pub is_hosting: bool,
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

pub fn get_public_ip_info() -> Result<IpInfo, String> {
    let config = ureq::config::Config::builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let resp = agent
        .get("https://ip-api.com/json/?fields=status,country,countryCode,region,regionName,city,isp,org,as,asname,proxy,hosting,query,reverse")
        .call()
        .map_err(|e| format!("Falha ao consultar ip-api: {}", e))?;

    let mut body = resp.into_body();
    let text = body
        .read_to_string()
        .map_err(|e| format!("Erro ao ler resposta: {}", e))?;

    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Erro ao parsear JSON: {}", e))?;

    if json.get("status").and_then(|v| v.as_str()) != Some("success") {
        return Err(format!("API retornou erro: {}", json));
    }

    let ipv4 = json["query"].as_str().unwrap_or("").to_string();
    let hostname = json["reverse"].as_str().map(|s| s.to_string());
    let country = json["country"].as_str().unwrap_or("").to_string();
    let country_code = json["countryCode"].as_str().unwrap_or("").to_string();
    let region = json["regionName"].as_str().unwrap_or("").to_string();
    let city = json["city"].as_str().unwrap_or("").to_string();
    let isp = json["isp"].as_str().unwrap_or("").to_string();
    let org = json["org"].as_str().unwrap_or("").to_string();
    let is_proxy = json["proxy"].as_bool().unwrap_or(false);
    let is_hosting = json["hosting"].as_bool().unwrap_or(false);

    let as_raw = json["as"].as_str().unwrap_or("");
    let as_name = json["asname"].as_str().unwrap_or("").to_string();
    let asn = if as_raw.starts_with("AS") {
        let parts: Vec<&str> = as_raw.splitn(2, ' ').collect();
        parts[0].to_string()
    } else {
        as_raw.to_string()
    };

    let ipv6 = get_ipv6_quiet();

    Ok(IpInfo {
        ipv4,
        ipv6,
        hostname,
        country,
        country_code,
        region,
        city,
        isp,
        org,
        asn,
        as_name,
        timezone: String::new(),
        is_proxy,
        is_hosting,
    })
}

fn get_ipv6_quiet() -> Option<String> {
    let config = ureq::config::Config::builder()
        .timeout_global(Some(std::time::Duration::from_secs(3)))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let resp = agent.get("https://api6.ipify.org").call().ok()?;
    let mut body = resp.into_body();
    let ip = body.read_to_string().ok()?;
    let ip = ip.trim().to_string();
    if ip.is_empty() { None } else { Some(ip) }
}
