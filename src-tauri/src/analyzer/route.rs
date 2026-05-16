use serde::Serialize;
use regex::Regex;

#[derive(Debug, Serialize)]
pub struct Hop {
    pub hop_number: u32,
    pub address: String,
    pub latency_ms: String,
}

#[derive(Debug, Serialize)]
pub struct PingResult {
    pub host: String,
    pub transmitted: u32,
    pub received: u32,
    pub loss_pct: u8,
    pub min_ms: f32,
    pub avg_ms: f32,
    pub max_ms: f32,
}

const MAX_HOPS: u32 = 30;

pub fn trace_route(host: &str) -> Result<Vec<Hop>, String> {
    if cfg!(target_os = "windows") {
        return trace_route_tracert(host);
    }

    let max_hops_str = MAX_HOPS.to_string();
    let cmd = "traceroute";
    let args = vec!["-n", "-m", &max_hops_str, host];

    match std::process::Command::new(cmd).args(&args).output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(hops) = parse_traceroute_output(&stdout) {
                return Ok(hops);
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return trace_route_tracepath(host);
        }
        _ => {}
    }

    trace_route_tracepath(host)
}

fn trace_route_tracert(host: &str) -> Result<Vec<Hop>, String> {
    let max_hops_str = MAX_HOPS.to_string();
    let output = std::process::Command::new("tracert")
        .args(["-h", &max_hops_str, "-d", host])
        .output()
        .map_err(|e| format!("Falha ao executar tracert: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_tracert_output(&stdout)
}

fn trace_route_tracepath(host: &str) -> Result<Vec<Hop>, String> {
    let output = std::process::Command::new("tracepath")
        .args(["-n", host])
        .output()
        .map_err(|e| format!("Falha ao executar tracepath: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_tracepath_output(&stdout)
}

pub fn parse_traceroute_output(output: &str) -> Result<Vec<Hop>, String> {
    let mut hops = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.chars().next().map_or(true, |c| c.is_ascii_digit()) {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(hop_num) = parts[0].parse::<u32>() {
                let addr = parts[1].trim_end_matches(':').to_string();
                let latencies: Vec<&str> = parts[2..]
                    .iter()
                    .filter(|s| **s != "*" && s.ends_with("ms"))
                    .copied()
                    .collect();

                hops.push(Hop {
                    hop_number: hop_num,
                    address: if addr == "*" { "*".to_string() } else { addr },
                    latency_ms: latencies.first().unwrap_or(&"*").to_string(),
                });
            }
        }
    }

    if hops.is_empty() {
        Err("Nenhum hop encontrado no traceroute".to_string())
    } else {
        Ok(hops)
    }
}

fn parse_tracert_output(output: &str) -> Result<Vec<Hop>, String> {
    let mut hops = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with(' ') {
            continue;
        }
        let line = line.trim();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(hop_num) = parts[0].parse::<u32>() {
                let addr = if parts[1] == "*" || parts.contains(&"Request") {
                    "*".to_string()
                } else {
                    parts[1].to_string()
                };
                hops.push(Hop {
                    hop_number: hop_num,
                    address: addr,
                    latency_ms: "0ms".to_string(),
                });
            }
        }
    }

    if hops.is_empty() {
        Err("Nenhum hop encontrado no tracert".to_string())
    } else {
        Ok(hops)
    }
}

pub fn parse_tracepath_output(output: &str) -> Result<Vec<Hop>, String> {
    let re = Regex::new(r"^\s*(\d+)\??:\s+(.+?)(?:\s+(\d+\.\d+ms|no reply))?\s*$").unwrap();
    let mut hops = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.contains("pmtu") || line.contains("too big") {
            continue;
        }

        if let Some(caps) = re.captures(line) {
            let hop_num: u32 = caps[1].parse().unwrap_or(0);
            let addr_raw = caps[2].trim();
            let addr = if addr_raw == "no reply" {
                "*".to_string()
            } else if addr_raw.starts_with("LOCAL") || addr_raw.starts_with('[') {
                format!("local ({})", addr_raw.trim_matches(|c| c == '[' || c == ']'))
            } else {
                addr_raw.to_string()
            };

            let latency = caps.get(3).map_or("?", |m| {
                let val = m.as_str();
                if val == "no reply" { "?" } else { val }
            });

            hops.push(Hop {
                hop_number: hop_num,
                address: addr,
                latency_ms: latency.to_string(),
            });
        }
    }

    if hops.is_empty() {
        Err("Nenhum hop encontrado no tracepath".to_string())
    } else {
        Ok(hops)
    }
}

pub fn ping_host(host: &str, count: u32) -> Result<PingResult, String> {
    let count_str = count.to_string();

    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "windows") {
        ("ping", vec!["-n", &count_str, host])
    } else {
        ("ping", vec!["-c", &count_str, host])
    };

    let output = std::process::Command::new(cmd)
        .args(&args)
        .output()
        .map_err(|e| format!("Falha ao executar ping: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Ping falhou: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ping_output(&stdout, count)
}

pub fn parse_ping_output(output: &str, count: u32) -> Result<PingResult, String> {
    let re_loss = Regex::new(r"(\d+)\s*%\s*(packet loss|perda de pacotes)").unwrap();
    let re_rtt = Regex::new(r"([\d.]+)/([\d.]+)/([\d.]+)").unwrap();

    let mut received: u32 = count;
    let mut loss_pct: u8 = 0;
    let mut min_ms: f32 = 0.0;
    let mut avg_ms: f32 = 0.0;
    let mut max_ms: f32 = 0.0;

    for line in output.lines() {
        if let Some(caps) = re_loss.captures(line) {
            loss_pct = caps[1].parse().unwrap_or(0);
            received = count.saturating_sub((loss_pct as f32 / 100.0 * count as f32).round() as u32);
        }

        if let Some(caps) = re_rtt.captures(line) {
            min_ms = caps[1].parse().unwrap_or(0.0);
            avg_ms = caps[2].parse().unwrap_or(0.0);
            max_ms = caps[3].parse().unwrap_or(0.0);
        }
    }

    let host = extract_host(output);

    Ok(PingResult {
        host,
        transmitted: count,
        received,
        loss_pct,
        min_ms,
        avg_ms,
        max_ms,
    })
}

fn extract_host(output: &str) -> String {
    for line in output.lines() {
        let line = line.trim();
        if line.starts_with("PING ") {
            let rest = &line[5..];
            if let Some(end) = rest.find(' ') {
                return rest[..end].to_string();
            }
            return rest.to_string();
        }
    }
    String::new()
}

pub fn classify_latency(ms: f32) -> &'static str {
    if ms < 5.0 { "Excelente" }
    else if ms < 30.0 { "Bom" }
    else if ms < 80.0 { "Aceitável" }
    else { "Ruim" }
}
