use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

pub const COMMON_PORTS: &[u16] = &[
    21, 22, 23, 25, 53, 80, 110, 111, 135, 139, 143, 443, 445, 465, 587, 993, 995, 1080, 1194,
    1433, 1521, 2049, 2375, 3000, 3306, 3389, 4444, 5432, 5900, 6379, 6881, 7070, 8080, 8443, 8888,
    9000, 9090, 9200, 10000, 11211, 27017, 27018, 50000, 51413, 52869, 55443, 60000,
];

fn build_service_map() -> HashMap<u16, &'static str> {
    let mut m = HashMap::new();
    m.insert(21, "FTP");
    m.insert(22, "SSH");
    m.insert(23, "Telnet");
    m.insert(25, "SMTP");
    m.insert(53, "DNS");
    m.insert(80, "HTTP");
    m.insert(110, "POP3");
    m.insert(111, "RPC");
    m.insert(135, "MSRPC");
    m.insert(139, "NetBIOS");
    m.insert(143, "IMAP");
    m.insert(443, "HTTPS");
    m.insert(445, "SMB");
    m.insert(465, "SMTPS");
    m.insert(587, "SMTP Alt");
    m.insert(993, "IMAPS");
    m.insert(995, "POP3S");
    m.insert(1080, "SOCKS");
    m.insert(1194, "OpenVPN");
    m.insert(1433, "MSSQL");
    m.insert(1521, "Oracle");
    m.insert(2049, "NFS");
    m.insert(2375, "Docker");
    m.insert(3000, "Dev");
    m.insert(3306, "MySQL");
    m.insert(3389, "RDP");
    m.insert(4444, "Metasploit");
    m.insert(5432, "PostgreSQL");
    m.insert(5900, "VNC");
    m.insert(6379, "Redis");
    m.insert(6881, "BitTorrent");
    m.insert(7070, "RTSP");
    m.insert(8080, "HTTP Alt");
    m.insert(8443, "HTTPS Alt");
    m.insert(8888, "Jupyter");
    m.insert(9000, "PHP-FPM");
    m.insert(9090, "Prometheus");
    m.insert(9200, "Elasticsearch");
    m.insert(10000, "Webmin");
    m.insert(11211, "Memcached");
    m.insert(27017, "MongoDB");
    m.insert(27018, "MongoDB Alt");
    m.insert(50000, "SAP");
    m.insert(51413, "Transmission");
    m.insert(52869, "UPnP");
    m.insert(55443, "App Alt");
    m.insert(60000, "App Alt");
    m
}

#[derive(Debug, Serialize)]
pub struct ListeningPort {
    pub port: u16,
    pub protocol: String,
    pub state: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResult {
    pub port: u16,
    pub service: String,
    pub state: String,
    pub response_ms: Option<f32>,
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
            if line.is_empty()
                || line.starts_with("Netid")
                || line.starts_with("ss:")
                || line.starts_with("State")
            {
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
    let service_map = build_service_map();

    for &port in ports {
        let addr = format!("{}:{}", host, port);

        let Ok(mut addrs) = addr.to_socket_addrs() else {
            results.push(ScanResult {
                port,
                service: service_name(port, &service_map),
                state: "ERRO".to_string(),
                response_ms: None,
            });
            continue;
        };

        let Some(socket_addr) = addrs.next() else {
            results.push(ScanResult {
                port,
                service: service_name(port, &service_map),
                state: "ERRO".to_string(),
                response_ms: None,
            });
            continue;
        };

        let start = Instant::now();
        match TcpStream::connect_timeout(&socket_addr, timeout) {
            Ok(_) => {
                let elapsed = start.elapsed().as_secs_f32() * 1000.0;
                results.push(ScanResult {
                    port,
                    service: service_name(port, &service_map),
                    state: "open".to_string(),
                    response_ms: Some(elapsed),
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                results.push(ScanResult {
                    port,
                    service: service_name(port, &service_map),
                    state: "filtered".to_string(),
                    response_ms: None,
                });
            }
            Err(_) => {
                results.push(ScanResult {
                    port,
                    service: service_name(port, &service_map),
                    state: "closed".to_string(),
                    response_ms: None,
                });
            }
        }
    }

    results
}

fn service_name(port: u16, map: &HashMap<u16, &'static str>) -> String {
    map.get(&port)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("TCP/{}", port))
}
