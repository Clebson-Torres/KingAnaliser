use serde::Serialize;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

pub const COMMON_PORTS: &[u16] = &[
    21, 22, 23, 25, 53, 80, 110, 111, 135, 139, 143, 443, 445, 993, 995, 1433, 1521, 2049, 3306,
    3389, 5432, 5900, 5985, 5986, 6379, 8080, 8443, 9090, 27017,
];

#[derive(Debug, Serialize)]
pub struct ListeningPort {
    pub port: u16,
    pub protocol: String,
    pub state: String,
}

#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub port: u16,
    pub service: String,
    pub state: String,
    pub latency_ms: u64,
}

pub fn get_port_list() -> Vec<u16> {
    COMMON_PORTS.to_vec()
}

pub fn get_listening_ports() -> Result<Vec<ListeningPort>, String> {
    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "windows") {
        ("netstat", vec!["-ano"])
    } else {
        ("ss", vec!["-tln4"])
    };

    let output = std::process::Command::new(cmd)
        .args(&args)
        .output()
        .map_err(|e| format!("Falha ao executar '{}': {}", cmd, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_listening_ports(&stdout)
}

fn parse_listening_ports(output: &str) -> Result<Vec<ListeningPort>, String> {
    let mut ports = Vec::new();

    if cfg!(target_os = "windows") {
        for line in output.lines() {
            let line = line.trim();
            if line.contains("LISTEN") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let addr_part = parts[1];
                    if let Some(port_str) = addr_part.rsplit(':').next() {
                        if let Ok(port) = port_str.parse::<u16>() {
                            ports.push(ListeningPort {
                                port,
                                protocol: if line.starts_with("TCP") {
                                    "TCP".to_string()
                                } else {
                                    "UDP".to_string()
                                },
                                state: "LISTEN".to_string(),
                            });
                        }
                    }
                }
            }
        }
    } else {
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("Netid") || line.starts_with("ss:") || line.starts_with("State") {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                let protocol = parts[0].to_string();
                let addr_part = parts[4];
                if let Some(port_str) = addr_part.rsplit(':').next() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        ports.push(ListeningPort {
                            port,
                            protocol: protocol.clone(),
                            state: "LISTEN".to_string(),
                        });
                    }
                }
            }
        }
    }

    ports.sort_by_key(|p| p.port);
    ports.dedup_by_key(|p| p.port);

    Ok(ports)
}

pub fn scan_ports(host: &str, ports: &[u16], timeout_ms: u64) -> Vec<ScanResult> {
    let timeout = Duration::from_millis(timeout_ms);
    let mut results = Vec::new();

    for &port in ports {
        let addr = format!("{}:{}", host, port);

        let start = Instant::now();
        let Ok(mut addrs) = addr.to_socket_addrs() else {
            results.push(ScanResult {
                port,
                service: service_name(port),
                state: "ERRO".to_string(),
                latency_ms: 0,
            });
            continue;
        };

        let Some(socket_addr) = addrs.next() else {
            results.push(ScanResult {
                port,
                service: service_name(port),
                state: "ERRO".to_string(),
                latency_ms: 0,
            });
            continue;
        };

        let state = match TcpStream::connect_timeout(&socket_addr, timeout) {
            Ok(_) => "ABERTA",
            Err(_) => "FECHADA",
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;

        results.push(ScanResult {
            port,
            service: service_name(port),
            state: state.to_string(),
            latency_ms: if state == "ABERTA" { elapsed_ms } else { 0 },
        });
    }

    results
}

fn service_name(port: u16) -> String {
    match port {
        21 => "FTP".to_string(),
        22 => "SSH".to_string(),
        23 => "Telnet".to_string(),
        25 => "SMTP".to_string(),
        53 => "DNS".to_string(),
        80 => "HTTP".to_string(),
        110 => "POP3".to_string(),
        111 => "RPC".to_string(),
        135 => "RPC".to_string(),
        139 => "NetBIOS".to_string(),
        143 => "IMAP".to_string(),
        443 => "HTTPS".to_string(),
        445 => "SMB".to_string(),
        993 => "IMAPS".to_string(),
        995 => "POP3S".to_string(),
        1433 => "MSSQL".to_string(),
        1521 => "Oracle".to_string(),
        2049 => "NFS".to_string(),
        3306 => "MySQL".to_string(),
        3389 => "RDP".to_string(),
        5432 => "PostgreSQL".to_string(),
        5900 => "VNC".to_string(),
        5985 => "WinRM-HTTP".to_string(),
        5986 => "WinRM-HTTPS".to_string(),
        6379 => "Redis".to_string(),
        8080 => "HTTP-Alt".to_string(),
        8443 => "HTTPS-Alt".to_string(),
        9090 => "HTTP-Alt2".to_string(),
        27017 => "MongoDB".to_string(),
        _ => format!("TCP/{}", port),
    }
}
