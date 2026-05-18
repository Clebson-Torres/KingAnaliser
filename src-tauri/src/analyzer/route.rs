use serde::{Deserialize, Serialize};
use regex::Regex;

#[derive(Debug, Serialize, Deserialize)]
pub struct Hop {
    pub hop_number: u32,
    pub address: String,
    pub hostname: Option<String>,
    pub avg_ms: f32,
    pub min_ms: f32,
    pub max_ms: f32,
    pub loss_pct: f32,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PingResult {
    pub host: String,
    pub packets_sent: u8,
    pub packets_received: u8,
    pub loss_pct: f32,
    pub min_ms: f32,
    pub avg_ms: f32,
    pub max_ms: f32,
    pub jitter_ms: f32,
    pub quality: String,
    pub quality_color: String,
}

const MAX_HOPS: u32 = 30;

pub fn ping_host(host: &str, count: u8) -> Result<PingResult, String> {
    let count_str = count.to_string();

    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "windows") {
        ("ping", vec!["-n", &count_str, host])
    } else {
        ("ping", vec!["-c", &count_str, "-i", "0.2", host])
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

pub fn parse_ping_output(output: &str, count: u8) -> Result<PingResult, String> {
    let host = extract_host(output);

    if cfg!(target_os = "windows") {
        return parse_ping_output_windows(output, count, host);
    }

    let re_loss = Regex::new(r"(\d+)\s+packets transmitted,\s+(\d+)\s+(received|packets received)").unwrap();
    let re_rtt = Regex::new(r"rtt min/avg/max/mdev\s*=\s*([\d.]+)/([\d.]+)/([\d.]+)/([\d.]+)").unwrap();

    let mut transmitted: u8 = count;
    let mut received: u8 = 0;
    let mut loss_pct: f32 = 0.0;
    let mut min_ms: f32 = 0.0;
    let mut avg_ms: f32 = 0.0;
    let mut max_ms: f32 = 0.0;
    let mut jitter_ms: f32 = 0.0;

    for line in output.lines() {
        if let Some(caps) = re_loss.captures(line) {
            transmitted = caps[1].parse().unwrap_or(count as u32) as u8;
            received = caps[2].parse().unwrap_or(0);
            loss_pct = if transmitted > 0 {
                ((transmitted - received) as f32 / transmitted as f32) * 100.0
            } else {
                0.0
            };
        }

        if let Some(caps) = re_rtt.captures(line) {
            min_ms = caps[1].parse().unwrap_or(0.0);
            avg_ms = caps[2].parse().unwrap_or(0.0);
            max_ms = caps[3].parse().unwrap_or(0.0);
            jitter_ms = caps[4].parse().unwrap_or(0.0);
        }
    }

    if transmitted == 0 {
        transmitted = count;
    }

    let (quality, quality_color) = classify_quality(avg_ms, loss_pct, jitter_ms);

    Ok(PingResult {
        host,
        packets_sent: transmitted,
        packets_received: received,
        loss_pct,
        min_ms,
        avg_ms,
        max_ms,
        jitter_ms,
        quality: quality.to_string(),
        quality_color: quality_color.to_string(),
    })
}

fn parse_ping_output_windows(output: &str, count: u8, host: String) -> Result<PingResult, String> {
    let re_loss = Regex::new(r"Enviados\s*=\s*(\d+),\s*Recebidos\s*=\s*(\d+),\s*Perdidos\s*=\s*\d+\s*\((\d+)%").unwrap();
    let re_loss_en = Regex::new(r"Sent\s*=\s*(\d+),\s*Received\s*=\s*(\d+),\s*Lost\s*=\s*\d+\s*\((\d+)%").unwrap();
    let re_rtt = Regex::new(r"M[íi]nimo\s*=\s*(\d+).*?M[áa]ximo\s*=\s*(\d+).*?M[ée]dia\s*=\s*(\d+)").unwrap();
    let re_rtt_en = Regex::new(r"Minimum\s*=\s*(\d+).*?Maximum\s*=\s*(\d+).*?Average\s*=\s*(\d+)").unwrap();

    let mut transmitted: u8 = count;
    let mut received: u8 = 0;
    let mut loss_pct: f32 = 0.0;
    let mut min_ms: f32 = 0.0;
    let mut avg_ms: f32 = 0.0;
    let mut max_ms: f32 = 0.0;

    for line in output.lines() {
        if let Some(caps) = re_loss.captures(line) {
            transmitted = caps[1].parse().unwrap_or(count as u32) as u8;
            received = caps[2].parse().unwrap_or(0);
            loss_pct = caps[3].parse().unwrap_or(0.0);
        } else if let Some(caps) = re_loss_en.captures(line) {
            transmitted = caps[1].parse().unwrap_or(count as u32) as u8;
            received = caps[2].parse().unwrap_or(0);
            loss_pct = caps[3].parse().unwrap_or(0.0);
        }

        if let Some(caps) = re_rtt.captures(line) {
            min_ms = caps[1].parse().unwrap_or(0.0);
            max_ms = caps[2].parse().unwrap_or(0.0);
            avg_ms = caps[3].parse().unwrap_or(0.0);
        } else if let Some(caps) = re_rtt_en.captures(line) {
            min_ms = caps[1].parse().unwrap_or(0.0);
            max_ms = caps[2].parse().unwrap_or(0.0);
            avg_ms = caps[3].parse().unwrap_or(0.0);
        }
    }

    let jitter_ms = if max_ms > min_ms { (max_ms - min_ms) / 2.0 } else { 0.0 };

    let (quality, quality_color) = classify_quality(avg_ms, loss_pct, jitter_ms);

    Ok(PingResult {
        host,
        packets_sent: transmitted,
        packets_received: received,
        loss_pct,
        min_ms,
        avg_ms,
        max_ms,
        jitter_ms,
        quality: quality.to_string(),
        quality_color: quality_color.to_string(),
    })
}

fn classify_quality(avg_ms: f32, loss_pct: f32, jitter_ms: f32) -> (&'static str, &'static str) {
    if avg_ms < 10.0 && loss_pct == 0.0 && jitter_ms < 5.0 {
        ("Excelente", "green")
    } else if avg_ms < 50.0 && loss_pct <= 1.0 && jitter_ms < 20.0 {
        ("Bom", "green")
    } else if avg_ms < 100.0 && loss_pct <= 5.0 {
        ("Aceitável", "yellow")
    } else {
        ("Ruim", "red")
    }
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

pub fn trace_route(host: &str) -> Result<Vec<Hop>, String> {
    if cfg!(target_os = "windows") {
        return trace_route_tracert(host);
    }

    let max_hops_str = MAX_HOPS.to_string();
    let cmd = "traceroute";
    let args = vec!["-n", "-q", "3", "-w", "1", "-m", &max_hops_str, host];

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

pub fn parse_traceroute_output(output: &str) -> Result<Vec<Hop>, String> {
    let mut hops = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.chars().next().map_or(true, |c| c.is_ascii_digit()) {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let Ok(hop_num) = parts[0].parse::<u32>() else { continue };
        let addr = parts[1].trim_end_matches(':').to_string();

        if addr == "*" || addr == "???" {
            hops.push(Hop {
                hop_number: hop_num,
                address: "*".to_string(),
                hostname: None,
                avg_ms: 0.0,
                min_ms: 0.0,
                max_ms: 0.0,
                loss_pct: 100.0,
                status: "no_reply".to_string(),
            });
            continue;
        }

        let rtts: Vec<f32> = parts.iter().skip(2)
            .filter(|s| s.ends_with("ms"))
            .filter_map(|s| s.trim_end_matches("ms").parse::<f32>().ok())
            .collect();

        let min_ms = rtts.iter().cloned().fold(f32::MAX, f32::min);
        let max_ms = rtts.iter().cloned().fold(0.0f32, f32::max);
        let avg_ms = if !rtts.is_empty() { rtts.iter().sum::<f32>() / rtts.len() as f32 } else { 0.0 };
        let loss_pct = if parts.len() > 2 {
            let total = parts[2..].len();
            let replies = rtts.len();
            if total > 0 { (total - replies) as f32 / total as f32 * 100.0 } else { 0.0 }
        } else {
            0.0
        };
        let min_ms = if min_ms == f32::MAX { 0.0 } else { min_ms };

        let hostname = resolve_hostname(&addr);

        let status = classify_hop_status(avg_ms, loss_pct, !rtts.is_empty());

        hops.push(Hop {
            hop_number: hop_num,
            address: addr,
            hostname,
            avg_ms,
            min_ms,
            max_ms,
            loss_pct,
            status,
        });
    }

    if hops.is_empty() {
        Err("Nenhum hop encontrado no traceroute".to_string())
    } else {
        Ok(hops)
    }
}

fn resolve_hostname(addr: &str) -> Option<String> {
    if addr == "*" || addr == "???" || addr.is_empty() {
        return None;
    }
    let ip: std::net::IpAddr = addr.parse().ok()?;
    match reverse_dns_std(&ip) {
        Some(name) if !name.is_empty() => Some(name),
        _ => None,
    }
}

fn reverse_dns_std(ip: &std::net::IpAddr) -> Option<String> {
    // Try reverse DNS via system command
    let ip_str = ip.to_string();
    if cfg!(target_os = "windows") {
        let output = std::process::Command::new("nslookup")
            .args([&ip_str])
            .output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("Name:") {
                if let Some(name) = line.split(':').nth(1) {
                    let name = name.trim().trim_end_matches('.').to_string();
                    if !name.is_empty() { return Some(name); }
                }
            }
        }
    } else {
        let output = std::process::Command::new("host")
            .args([&ip_str])
            .output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(pos) = line.find("domain name pointer") {
                let rest = &line[pos + "domain name pointer".len()..];
                let name = rest.trim().trim_end_matches('.').to_string();
                if !name.is_empty() { return Some(name); }
            }
        }
    }
    None
}

fn classify_hop_status(avg_ms: f32, loss_pct: f32, has_reply: bool) -> String {
    if !has_reply {
        return "no_reply".to_string();
    }
    if avg_ms < 30.0 && loss_pct == 0.0 {
        "ok".to_string()
    } else if avg_ms < 80.0 || loss_pct <= 5.0 {
        "warning".to_string()
    } else {
        "critical".to_string()
    }
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

fn parse_tracert_output(output: &str) -> Result<Vec<Hop>, String> {
    let mut hops = Vec::new();
    let _re = Regex::new(r"^\s*(\d+)\s+(.*?)(\d+\.?\d*\s*ms|\*)\s*.*$").unwrap();
    let re_line = Regex::new(r"^\s*(\d+)").unwrap();

    for line in output.lines() {
        if !re_line.is_match(line) { continue; }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 { continue; }

        let Ok(hop_num) = parts[0].parse::<u32>() else { continue };

        if parts[1] == "*" || line.contains("Tempo limite") || line.contains("Request timed out") {
            hops.push(Hop {
                hop_number: hop_num,
                address: "*".to_string(),
                hostname: None,
                avg_ms: 0.0,
                min_ms: 0.0,
                max_ms: 0.0,
                loss_pct: 100.0,
                status: "no_reply".to_string(),
            });
            continue;
        }

        let mut rtts: Vec<f32> = parts[1..]
            .iter()
            .filter_map(|s| {
                let s = s.trim_end_matches("ms");
                if s == "<1" || s == "<" {
                    Some(0.5f32)
                } else {
                    s.parse::<f32>().ok()
                }
            })
            .collect();

        let addr_idx = rtts.len() + 1;
        let addr = if addr_idx < parts.len() {
            parts[addr_idx].to_string()
        } else {
            parts.last().unwrap_or(&"*").to_string()
        };

        if rtts.is_empty() {
            rtts.push(0.0);
        }

        let min_ms = rtts.iter().cloned().fold(f32::MAX, f32::min);
        let max_ms = rtts.iter().cloned().fold(0.0f32, f32::max);
        let avg_ms = rtts.iter().sum::<f32>() / rtts.len() as f32;
        let min_ms = if min_ms == f32::MAX { 0.0 } else { min_ms };
        let loss_pct = 0.0;

        let hostname = resolve_hostname(&addr);
        let status = classify_hop_status(avg_ms, loss_pct, true);

        hops.push(Hop {
            hop_number: hop_num,
            address: addr,
            hostname,
            avg_ms,
            min_ms,
            max_ms,
            loss_pct,
            status,
        });
    }

    if hops.is_empty() {
        Err("Nenhum hop encontrado no tracert".to_string())
    } else {
        Ok(hops)
    }
}

fn trace_route_tracepath(host: &str) -> Result<Vec<Hop>, String> {
    let output = std::process::Command::new("tracepath")
        .args(["-n", host])
        .output()
        .map_err(|e| format!("Falha ao executar tracepath: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_tracepath_output(&stdout)
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
            let addr = if addr_raw == "no reply" || addr_raw.starts_with("LOCAL") || addr_raw.starts_with('[') {
                "*".to_string()
            } else {
                addr_raw.to_string()
            };

            let has_reply = caps.get(3).map_or(false, |m| m.as_str() != "no reply" && !m.as_str().is_empty());

            let hostname = resolve_hostname(&addr);

            hops.push(Hop {
                hop_number: hop_num,
                address: addr,
                hostname,
                avg_ms: 0.0,
                min_ms: 0.0,
                max_ms: 0.0,
                loss_pct: if has_reply { 0.0 } else { 100.0 },
                status: if has_reply { "ok".to_string() } else { "no_reply".to_string() },
            });
        }
    }

    if hops.is_empty() {
        Err("Nenhum hop encontrado no tracepath".to_string())
    } else {
        Ok(hops)
    }
}

#[allow(dead_code)]
pub fn classify_latency(ms: f32) -> &'static str {
    if ms < 5.0 { "Excelente" }
    else if ms < 30.0 { "Bom" }
    else if ms < 80.0 { "Aceitável" }
    else { "Ruim" }
}
