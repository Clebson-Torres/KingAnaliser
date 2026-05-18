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

fn benchmark_dns_linux(ip: &str) -> (u32, String) {
    let output = std::process::Command::new("dig")
        .args(["@", ip, "google.com", "+stats"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                if line.contains("Query time:") {
                    if let Some(ms_str) = line.split_whitespace().nth(3) {
                        if let Ok(ms) = ms_str.parse::<u32>() {
                            return (ms, "OK".to_string());
                        }
                    }
                }
            }
            (0, "Falha ao parsear resposta".to_string())
        }
        _ => {
            let fallback = benchmark_dns_nslookup(ip);
            if fallback.0 > 0 {
                fallback
            } else {
                (0, "Falha ao executar dig".to_string())
            }
        }
    }
}

fn benchmark_dns_nslookup(ip: &str) -> (u32, String) {
    let output = std::process::Command::new("nslookup")
        .args(["google.com", ip])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                if line.contains("msec") {
                    if let Some(ms_str) = line.split_whitespace().find(|s| s.ends_with("msec")) {
                        if let Ok(ms) = ms_str.trim_end_matches("msec").parse::<u32>() {
                            return (ms, "OK".to_string());
                        }
                    }
                }
            }
            (0, "Falha ao parsear nslookup".to_string())
        }
        _ => (0, "Falha ao executar nslookup".to_string()),
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
