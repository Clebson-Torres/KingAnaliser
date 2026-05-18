use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkHost {
    pub ip: String,
    pub hostname: Option<String>,
    pub mac: Option<String>,
    pub vendor: Option<String>,
    pub latency_ms: Option<f32>,
    pub is_gateway: bool,
    pub open_ports: Vec<u16>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkScanResult {
    pub subnet: String,
    pub total_hosts: u16,
    pub hosts_up: u16,
    pub scan_duration_secs: f32,
    pub hosts: Vec<NetworkHost>,
}

const SCAN_PORTS: &[u16] = &[22, 80, 443, 3389, 8080];

fn build_oui_map() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("00:50:56", "VMware");
    m.insert("B8:27:EB", "Raspberry Pi");
    m.insert("DC:A6:32", "Raspberry Pi");
    m.insert("00:0C:29", "VMware");
    m.insert("FC:AA:14", "Amazon");
    m.insert("00:1A:11", "Google");
    m.insert("AC:DE:48", "Apple");
    m.insert("3C:22:FB", "Apple");
    m.insert("00:1B:63", "Apple");
    m.insert("00:11:32", "Synology");
    m.insert("00:17:88", "Philips Hue");
    m.insert("B4:E6:2D", "Ubiquiti");
    m.insert("DC:9F:DB", "Ubiquiti");
    m.insert("78:8A:20", "Ubiquiti");
    m.insert("00:18:0A", "TP-Link");
    m.insert("50:C7:BF", "TP-Link");
    m.insert("00:50:43", "Cisco");
    m.insert("00:1E:13", "Cisco");
    m
}

pub fn scan_network(subnet: Option<String>) -> Result<NetworkScanResult, String> {
    let start = Instant::now();

    let (base_ip, prefix) = detect_subnet(subnet)?;
    if prefix != 24 {
        return Err("No momento o scan de rede aceita apenas sub-redes /24".to_string());
    }

    let gateway_ip = get_gateway_ip_quiet();
    let oui_map = build_oui_map();
    let arp_table = get_arp_table();

    let total_hosts = 254u16;
    let concurrency = thread::available_parallelism()
        .map(|n| (n.get() * 8).clamp(16, 64))
        .unwrap_or(32);

    let mut hosts = Vec::new();
    for chunk_start in (1..=254).step_by(concurrency) {
        let chunk_end = (chunk_start + concurrency - 1).min(254);
        let mut chunk_hosts = thread::scope(|scope| {
            let mut handles = Vec::new();
            for i in chunk_start..=chunk_end {
                let ip = format!("{}.{}", base_ip, i);
                let gateway_ip = gateway_ip.as_deref();
                let arp_table = &arp_table;
                let oui_map = &oui_map;
                handles.push(scope.spawn(move || scan_host(ip, gateway_ip, arp_table, oui_map)));
            }

            handles
                .into_iter()
                .filter_map(|handle| handle.join().ok().flatten())
                .collect::<Vec<_>>()
        });
        hosts.append(&mut chunk_hosts);
    }

    hosts.sort_by_key(|h| {
        h.ip.rsplit('.')
            .next()
            .and_then(|n| n.parse::<u16>().ok())
            .unwrap_or(0)
    });

    let hosts_up = hosts.len() as u16;
    let scan_duration_secs = start.elapsed().as_secs_f32();

    Ok(NetworkScanResult {
        subnet: format!("{}.0/{}", base_ip, 24),
        total_hosts,
        hosts_up,
        scan_duration_secs,
        hosts,
    })
}

fn scan_host(
    ip: String,
    gateway_ip: Option<&str>,
    arp_table: &HashMap<String, String>,
    oui_map: &HashMap<&str, &str>,
) -> Option<NetworkHost> {
    let is_gateway = gateway_ip == Some(ip.as_str());
    let is_up = check_host_up(&ip);

    if !is_up && !is_gateway {
        return None;
    }

    let hostname = if is_up { resolve_hostname(&ip) } else { None };
    let mac = arp_table.get(&ip).cloned();
    let vendor = mac.as_ref().and_then(|m| identify_vendor(m, oui_map));
    let latency_ms = if is_up { measure_latency(&ip) } else { None };

    let open_ports = if is_up {
        scan_quick_ports(&ip, SCAN_PORTS)
    } else {
        Vec::new()
    };

    Some(NetworkHost {
        ip,
        hostname,
        mac,
        vendor,
        latency_ms,
        is_gateway,
        open_ports,
    })
}

pub(crate) fn detect_subnet(subnet: Option<String>) -> Result<(String, u8), String> {
    if let Some(s) = subnet {
        let s = s.trim();
        if let Some(pos) = s.find('/') {
            let base = s[..pos].trim_end_matches('.').to_string();
            let prefix: u8 = s[pos + 1..]
                .parse()
                .map_err(|_| "Prefixo CIDR inválido".to_string())?;
            let parts: Vec<&str> = base.split('.').collect();
            if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
                return Ok((format!("{}.{}.{}", parts[0], parts[1], parts[2]), prefix));
            }
            return Err("Sub-rede inválida. Use o formato 192.168.1.0/24".to_string());
        }
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
            return Ok((format!("{}.{}.{}", parts[0], parts[1], parts[2]), 24));
        }
        return Err("Sub-rede inválida. Use o formato 192.168.1.0/24".to_string());
    }

    let output = std::process::Command::new("ip")
        .args(["route", "show"])
        .output()
        .map_err(|e| format!("Falha ao executar ip route: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(pos) = line.find('/') {
            let ip_part = line[..pos].trim();
            let parts: Vec<&str> = ip_part.split('.').collect();
            if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
                return Ok((format!("{}.{}.{}", parts[0], parts[1], parts[2]), 24));
            }
        }
    }

    Err("Nao foi possivel detectar a subnet automaticamente".to_string())
}

fn get_gateway_ip_quiet() -> Option<String> {
    let output = std::process::Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().next().and_then(|line| {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 && parts[0] == "default" {
            Some(parts[2].to_string())
        } else {
            None
        }
    })
}

fn check_host_up(ip: &str) -> bool {
    let small_timeout = Duration::from_millis(300);

    for &port in &[80, 443] {
        let addr = format!("{}:{}", ip, port);
        if let Ok(mut addrs) = addr.to_socket_addrs() {
            if let Some(sa) = addrs.next() {
                if TcpStream::connect_timeout(&sa, small_timeout).is_ok() {
                    return true;
                }
            }
        }
    }

    // Fallback: ping with quick timeout
    let output = if cfg!(target_os = "windows") {
        std::process::Command::new("ping")
            .args(["-n", "1", "-w", "500", ip])
            .output()
    } else {
        std::process::Command::new("ping")
            .args(["-c", "1", "-W", "1", ip])
            .output()
    };

    match output {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

fn resolve_hostname(ip: &str) -> Option<String> {
    // Try reverse DNS via system command
    if cfg!(target_os = "windows") {
        let output = std::process::Command::new("nslookup")
            .args([ip])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("Name:") {
                if let Some(name) = line.split(':').nth(1) {
                    let name = name.trim().trim_end_matches('.').to_string();
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
    } else {
        let output = std::process::Command::new("host")
            .args([ip])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(pos) = line.find("domain name pointer") {
                let rest = &line[pos + "domain name pointer".len()..];
                let name = rest.trim().trim_end_matches('.').to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }
    None
}

fn get_arp_table() -> HashMap<String, String> {
    let mut map = HashMap::new();

    if cfg!(target_os = "windows") {
        if let Ok(output) = std::process::Command::new("arp").args(["-a"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3
                    && parts[0]
                        .chars()
                        .next()
                        .map_or(false, |c| c.is_ascii_digit())
                {
                    let ip = parts[0].trim().to_string();
                    let mac = parts[1].trim().to_string();
                    if mac != "ff-ff-ff-ff-ff-ff" && mac != "00-00-00-00-00-00" {
                        map.insert(ip, mac.replace('-', ":"));
                    }
                }
            }
        }
    } else {
        let content = std::fs::read_to_string("/proc/net/arp").ok();
        if let Some(data) = content {
            for (i, line) in data.lines().enumerate() {
                if i == 0 {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let ip = parts[0].to_string();
                    let mac = parts[3].to_string();
                    if mac != "00:00:00:00:00:00" && mac.chars().any(|c| c != '0' && c != ':') {
                        map.insert(ip, mac);
                    }
                }
            }
        }
    }

    map
}

fn identify_vendor(mac: &str, oui_map: &HashMap<&str, &str>) -> Option<String> {
    let oui = mac.to_uppercase();
    let prefix = if oui.len() >= 8 {
        &oui[..8]
    } else {
        return None;
    };

    oui_map.get(prefix).map(|v| v.to_string())
}

fn measure_latency(ip: &str) -> Option<f32> {
    let output = if cfg!(target_os = "windows") {
        std::process::Command::new("ping")
            .args(["-n", "1", "-w", "2000", ip])
            .output()
    } else {
        std::process::Command::new("ping")
            .args(["-c", "1", "-W", "2", ip])
            .output()
    };

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                if cfg!(target_os = "windows") {
                    if let Some(pos) = line.find("tempo=") {
                        let rest = &line[pos + 6..];
                        if let Some(end) = rest.find('m') {
                            return rest[..end].trim().parse::<f32>().ok();
                        }
                    }
                    if let Some(pos) = line.find("time=") {
                        let rest = &line[pos + 5..];
                        if let Some(end) = rest.find('m') {
                            return rest[..end].trim().parse::<f32>().ok();
                        }
                    }
                } else {
                    if let Some(pos) = line.find("time=") {
                        let rest = &line[pos + 5..];
                        if let Some(end) = rest.find("ms") {
                            return rest[..end].trim().parse::<f32>().ok();
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn scan_quick_ports(ip: &str, ports: &[u16]) -> Vec<u16> {
    let timeout = Duration::from_millis(200);
    let mut open = Vec::new();

    for &port in ports {
        let addr = format!("{}:{}", ip, port);
        if let Ok(mut addrs) = addr.to_socket_addrs() {
            if let Some(sa) = addrs.next() {
                if TcpStream::connect_timeout(&sa, timeout).is_ok() {
                    open.push(port);
                }
            }
        }
    }

    open
}
