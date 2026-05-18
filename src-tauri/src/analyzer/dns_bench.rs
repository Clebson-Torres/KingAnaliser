use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DnsServer {
    pub ip: String,
    pub name: String,
    pub latency_ms: u32,
    pub status: String,
    pub best: bool,
}

const DNS_TARGETS: &[(&str, &str)] = &[
    ("8.8.8.8", "Google DNS"),
    ("1.1.1.1", "Cloudflare DNS"),
    ("8.8.4.4", "Google DNS 2"),
    ("208.67.222.222", "OpenDNS"),
];

pub fn benchmark_dns() -> Vec<DnsServer> {
    let mut results: Vec<DnsServer> = DNS_TARGETS
        .iter()
        .map(|(ip, name)| {
            let (latency, status) = if cfg!(target_os = "windows") {
                benchmark_dns_windows(ip)
            } else {
                benchmark_dns_linux(ip)
            };
            DnsServer {
                ip: ip.to_string(),
                name: name.to_string(),
                latency_ms: latency,
                status,
                best: false,
            }
        })
        .collect();

    results.sort_by_key(|r| r.latency_ms);
    if let Some(first) = results.first_mut() {
        if first.status == "OK" {
            first.best = true;
        }
    }
    results
}

fn is_tool_available(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn benchmark_dns_dig(ip: &str) -> Option<(u32, String)> {
    if !is_tool_available("dig") {
        return None;
    }
    let dig_arg = format!("@{}", ip);
    let output = std::process::Command::new("dig")
        .args([&dig_arg, "google.com", "+stats"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("Query time:") {
            if let Some(ms_str) = line.split_whitespace().nth(3) {
                if let Ok(ms) = ms_str.parse::<u32>() {
                    return Some((ms, "OK".to_string()));
                }
            }
        }
    }
    None
}

fn benchmark_dns_resolvectl(_ip: &str) -> Option<(u32, String)> {
    if !is_tool_available("resolvectl") {
        return None;
    }
    let start = std::time::Instant::now();
    let output = std::process::Command::new("resolvectl")
        .args(["query", "google.com"])
        .output()
        .ok()?;
    let elapsed = start.elapsed().as_millis() as u32;
    if output.status.success() {
        Some((elapsed, "OK".to_string()))
    } else {
        None
    }
}

fn benchmark_dns_getent(_ip: &str) -> Option<(u32, String)> {
    if !is_tool_available("getent") {
        return None;
    }
    // getent doesn't support custom DNS servers, so we time it as a reference
    let start = std::time::Instant::now();
    let output = std::process::Command::new("getent")
        .args(["hosts", "google.com"])
        .output()
        .ok()?;
    let elapsed = start.elapsed().as_millis() as u32;
    if output.status.success() {
        Some((elapsed, "OK (default DNS)".to_string()))
    } else {
        None
    }
}

fn benchmark_dns_linux(ip: &str) -> (u32, String) {
    // Try dig (specifically queries the target DNS server)
    if let Some(result) = benchmark_dns_dig(ip) {
        return result;
    }

    // Try nslookup (also targets a specific DNS server)
    let result = benchmark_dns_nslookup(ip);
    if result.0 > 0 {
        return result;
    }

    // Fallback: use resolvectl or getent (system default DNS)
    if let Some(result) = benchmark_dns_resolvectl(ip) {
        return result;
    }
    if let Some(result) = benchmark_dns_getent(ip) {
        return result;
    }

    // Last resort: direct TCP/53 connection timing
    benchmark_dns_tcp(ip)
}

fn benchmark_dns_nslookup(ip: &str) -> (u32, String) {
    let output = std::process::Command::new("nslookup")
        .args(["google.com", ip])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                // Linux: "Query time: 12 msec" or "12 msec"
                if line.contains("Query time:") {
                    if let Some(ms_str) = line.split_whitespace().nth(3) {
                        if let Ok(ms) = ms_str.parse::<u32>() {
                            return (ms, "OK".to_string());
                        }
                    }
                }
                // Also try: "word 12 msec" pattern
                if line.contains("msec") {
                    for (i, word) in line.split_whitespace().enumerate() {
                        if word == "msec" {
                            if let Some(prev) = line.split_whitespace().nth(i.saturating_sub(1)) {
                                if let Ok(ms) = prev.parse::<u32>() {
                                    return (ms, "OK".to_string());
                                }
                            }
                        }
                    }
                }
            }
            (0, "Falha ao parsear nslookup".to_string())
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            (0, format!("nslookup: {}", stderr.trim()))
        }
        Err(e) => (0, format!("nslookup: {}", e)),
    }
}

fn benchmark_dns_tcp(ip: &str) -> (u32, String) {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;

    let addr_str = format!("{}:53", ip);
    let addr = match addr_str.to_socket_addrs() {
        Ok(mut addrs) => addrs.next(),
        Err(_) => return (0, "Resolução de endereço falhou".to_string()),
    };

    let Some(addr) = addr else {
        return (0, "Resolução de endereço falhou".to_string());
    };

    let start = std::time::Instant::now();
    match TcpStream::connect_timeout(&addr, Duration::from_millis(3000)) {
        Ok(stream) => {
            let elapsed = start.elapsed().as_millis() as u32;
            drop(stream); // Close immediately
            (elapsed, "OK (TCP/53)".to_string())
        }
        Err(_) => (0, "Sem resposta DNS".to_string()),
    }
}

fn benchmark_dns_windows(ip: &str) -> (u32, String) {
    let script = format!(
        "$sw = [Diagnostics.Stopwatch]::StartNew(); \
         Resolve-DnsName google.com -Server {} | Out-Null; \
         $sw.Stop(); \
         Write-Output $sw.ElapsedMilliseconds",
        ip
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let ms = stdout.trim().parse::<u32>().unwrap_or(0);
            if ms > 0 {
                (ms, "OK".to_string())
            } else {
                (0, "Falha ao medir latência".to_string())
            }
        }
        _ => (0, "Falha ao executar PowerShell".to_string()),
    }
}
