use regex::Regex;
use serde::{Deserialize, Serialize};

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
    } else if count == 1 {
        ("ping", vec!["-c", "1", host])
    } else {
        ("ping", vec!["-c", &count_str, "-i", "0.2", host])
    };

    let output = crate::process::command(cmd)
        .args(&args)
        .output()
        .map_err(|e| format!("Falha ao executar ping: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_trimmed = stderr.trim();
        // Some systems output ping stats even when the process exits with error
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() && stdout.contains("packets transmitted") {
            if let Ok(result) = parse_ping_output(&stdout, count) {
                return Ok(result);
            }
        }
        return Err(format!(
            "Ping falhou: {}",
            if stderr_trimmed.is_empty() {
                "host inalcançável ou bloqueado"
            } else {
                stderr_trimmed
            }
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ping_output(&stdout, count)
}

pub fn parse_ping_output(output: &str, count: u8) -> Result<PingResult, String> {
    let host = extract_host(output);

    let looks_like_windows = output.contains("Enviados")
        || output.contains("Recebidos")
        || output.contains("Sent =")
        || output.contains("Received =")
        || output.contains("Minimum =")
        || output.contains("Mínimo")
        || output.contains("Minimo");

    if looks_like_windows {
        return parse_ping_output_windows(output, count, host);
    }

    let re_loss = Regex::new(
        r"(?i)(\d+)\s+(packets transmitted|pacotes transmitidos),\s+(\d+)\s+(received|packets received|recebidos)",
    )
    .unwrap();
    let re_rtt =
        Regex::new(r"rtt min/avg/max/mdev\s*=\s*([\d.]+)/([\d.]+)/([\d.]+)/([\d.]+)").unwrap();

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
            received = caps[3].parse().unwrap_or(0);
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
    let re_loss = Regex::new(
        r"Enviados\s*=\s*(\d+),\s*Recebidos\s*=\s*(\d+),\s*Perdidos\s*=\s*\d+\s*\((\d+)%",
    )
    .unwrap();
    let re_loss_en =
        Regex::new(r"Sent\s*=\s*(\d+),\s*Received\s*=\s*(\d+),\s*Lost\s*=\s*\d+\s*\((\d+)%")
            .unwrap();
    let re_rtt = Regex::new(r"(?i)(m.nimo|minimum)\s*=\s*(\d+).*?(m.ximo|maximum)\s*=\s*(\d+).*?(m.dia|average)\s*=\s*(\d+)").unwrap();
    let re_rtt_en =
        Regex::new(r"Minimum\s*=\s*(\d+).*?Maximum\s*=\s*(\d+).*?Average\s*=\s*(\d+)").unwrap();

    let host = if host.is_empty() {
        extract_windows_host(output)
    } else {
        host
    };
    let mut transmitted: u8 = count;
    let mut received: u8 = 0;
    let mut loss_pct: f32 = 0.0;
    let mut min_ms: f32 = 0.0;
    let mut avg_ms: f32 = 0.0;
    let mut max_ms: f32 = 0.0;
    let mut samples = Vec::new();

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
            min_ms = caps[2].parse().unwrap_or(0.0);
            max_ms = caps[4].parse().unwrap_or(0.0);
            avg_ms = caps[6].parse().unwrap_or(0.0);
        } else if let Some(caps) = re_rtt_en.captures(line) {
            min_ms = caps[1].parse().unwrap_or(0.0);
            max_ms = caps[2].parse().unwrap_or(0.0);
            avg_ms = caps[3].parse().unwrap_or(0.0);
        }

        if let Some(ms) = ping_continuous_parse_line(line) {
            samples.push(ms);
        }
    }

    if avg_ms == 0.0 && !samples.is_empty() {
        min_ms = samples.iter().copied().fold(f32::MAX, f32::min);
        max_ms = samples.iter().copied().fold(0.0, f32::max);
        avg_ms = samples.iter().sum::<f32>() / samples.len() as f32;
        if min_ms == f32::MAX {
            min_ms = 0.0;
        }
    }

    if received == 0 && !samples.is_empty() {
        received = samples.len().min(u8::MAX as usize) as u8;
        transmitted = transmitted.max(received);
        loss_pct = if transmitted > 0 {
            ((transmitted - received) as f32 / transmitted as f32) * 100.0
        } else {
            0.0
        };
    }

    let jitter_ms = if max_ms > min_ms {
        (max_ms - min_ms) / 2.0
    } else {
        0.0
    };

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

fn extract_windows_host(output: &str) -> String {
    for line in output.lines() {
        let line = line.trim();
        for prefix in ["Disparando ", "Pinging "] {
            if let Some(rest) = line.strip_prefix(prefix) {
                return rest
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_matches(['[', ']'])
                    .to_string();
            }
        }

        for marker in ["Ping statistics for ", "Estat"] {
            if let Some(pos) = line.find(marker) {
                let rest = if marker == "Estat" {
                    line.split(" para ").nth(1).unwrap_or("")
                } else {
                    &line[pos + marker.len()..]
                };
                let host = rest.trim().trim_end_matches(':').trim_matches(['[', ']']);
                if !host.is_empty() {
                    return host.to_string();
                }
            }
        }
    }
    String::new()
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

    let has_traceroute = crate::process::command("which")
        .arg("traceroute")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let mut best_hops: Option<Vec<Hop>> = None;
    let mut errors = Vec::new();

    if has_traceroute {
        let max_hops_str = MAX_HOPS.to_string();
        let candidates: Vec<Vec<&str>> = vec![
            vec!["-n", "-q", "3", "-w", "2", "-m", &max_hops_str, host],
            vec![
                "-T",
                "-p",
                "443",
                "-n",
                "-q",
                "3",
                "-w",
                "2",
                "-m",
                &max_hops_str,
                host,
            ],
            vec!["-I", "-n", "-q", "3", "-w", "2", "-m", &max_hops_str, host],
        ];

        for args in candidates {
            match crate::process::command("traceroute").args(&args).output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if let Ok(hops) = parse_traceroute_output(&stdout) {
                        if route_response_score(&hops)
                            > route_response_score(best_hops.as_deref().unwrap_or(&[]))
                        {
                            best_hops = Some(hops);
                        }
                    }
                    if !stderr.trim().is_empty() {
                        errors.push(stderr.trim().to_string());
                    }
                }
                Err(e) => {
                    errors.push(e.to_string());
                }
            }
        }
    }

    if let Ok(hops) = trace_route_tracepath(host) {
        if route_response_score(&hops) > route_response_score(best_hops.as_deref().unwrap_or(&[])) {
            best_hops = Some(hops);
        }
    }

    if let Some(hops) = best_hops {
        if !hops.is_empty() {
            return Ok(trim_trailing_no_reply(hops));
        }
    }

    let detail = if errors.is_empty() {
        "sem resposta dos comandos de rota".to_string()
    } else {
        errors.join("; ")
    };
    Err(format!(
        "Falha ao executar traceroute/tracepath: {}",
        detail
    ))
}

fn route_response_score(hops: &[Hop]) -> usize {
    hops.iter()
        .filter(|h| h.address != "*" && h.status != "no_reply")
        .count()
}

fn trim_trailing_no_reply(mut hops: Vec<Hop>) -> Vec<Hop> {
    while hops.len() > 1 && hops.last().map_or(false, |h| h.status == "no_reply") {
        hops.pop();
    }
    hops
}

fn strip_ansi(s: &str) -> String {
    let re = regex::Regex::new(r"\x1B\[[0-9;]*[a-zA-Z]").unwrap();
    re.replace_all(s, "").to_string()
}

pub fn parse_traceroute_output(raw: &str) -> Result<Vec<Hop>, String> {
    let output = strip_ansi(raw);
    let mut hops = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Skip header lines
        let first = line.chars().next();
        if first.map_or(true, |c| !c.is_ascii_digit() && c != ' ') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let Ok(hop_num) = parts[0].parse::<u32>() else {
            continue;
        };

        // Check if entire hop is no reply
        if parts
            .iter()
            .skip(1)
            .all(|p| *p == "*" || *p == "???" || p.ends_with("ms"))
        {
            // Might be "* * *" which means no reply
            if parts[1] == "*" {
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
        }

        // Find the address - it's the first non-numeric token after hop number
        // that isn't an RTT value
        let mut addr_idx = 1;
        let mut addr = String::new();
        for (i, part) in parts.iter().enumerate().skip(1) {
            if *part == "*" || *part == "???" {
                addr = "*".to_string();
                addr_idx = i;
                break;
            }
            // Check if it looks like an IP or hostname (not a number with possible decimal)
            if !part.parse::<f32>().is_ok() && !part.ends_with("ms") {
                addr = part.to_string();
                addr_idx = i;
                break;
            }
        }

        if addr.is_empty() || addr == "*" {
            hops.push(Hop {
                hop_number: hop_num,
                address: if addr.is_empty() {
                    "*".to_string()
                } else {
                    addr
                },
                hostname: None,
                avg_ms: 0.0,
                min_ms: 0.0,
                max_ms: 0.0,
                loss_pct: 100.0,
                status: "no_reply".to_string(),
            });
            continue;
        }

        // Collect RTT values - look for pairs of (number, "ms") after the address
        let rtts: Vec<f32> = parts[addr_idx + 1..]
            .windows(2)
            .filter(|pair| pair[1] == "ms" || pair[1].starts_with("ms"))
            .filter_map(|pair| {
                let val = pair[0].trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.');
                if val == "<1" || val == "<" {
                    return Some(0.5);
                }
                val.parse::<f32>().ok()
            })
            .collect();

        let min_ms = rtts.iter().cloned().fold(f32::MAX, f32::min);
        let max_ms = rtts.iter().cloned().fold(0.0f32, f32::max);
        let avg_ms = if !rtts.is_empty() {
            rtts.iter().sum::<f32>() / rtts.len() as f32
        } else {
            0.0
        };

        // Calculate loss for this hop
        let expected = parts[addr_idx + 1..]
            .iter()
            .filter(|s| **s != "ms" && s.ends_with("ms") || s.parse::<f32>().is_ok())
            .count();
        let loss_pct = if expected > 0 {
            (expected - rtts.len()) as f32 / expected as f32 * 100.0
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
        let output = crate::process::command("nslookup")
            .args([&ip_str])
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
        let output = crate::process::command("host")
            .args([&ip_str])
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
    let output = crate::process::command("tracert")
        .args(["-h", &max_hops_str, "-w", "3000", "-d", host])
        .output()
        .map_err(|e| format!("Falha ao executar tracert: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_tracert_output(&stdout)
}

pub(crate) fn parse_tracert_output(output: &str) -> Result<Vec<Hop>, String> {
    let mut hops = Vec::new();
    let re_line = Regex::new(r"^\s*(\d+)").unwrap();

    for line in output.lines() {
        if !re_line.is_match(line) {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let Ok(hop_num) = parts[0].parse::<u32>() else {
            continue;
        };

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

        let mut rtts = Vec::new();
        let mut i = 1;
        while i < parts.len() {
            let token = parts[i].trim();
            let next = parts.get(i + 1).map(|s| s.trim());

            if token == "<1" && next == Some("ms") {
                rtts.push(0.5);
                i += 2;
                continue;
            }

            if let Some(raw) = token.strip_suffix("ms") {
                if raw == "<1" {
                    rtts.push(0.5);
                } else if let Ok(ms) = raw.parse::<f32>() {
                    rtts.push(ms);
                }
                i += 1;
                continue;
            }

            if next == Some("ms") {
                if let Ok(ms) = token.parse::<f32>() {
                    rtts.push(ms);
                    i += 2;
                    continue;
                }
            }

            i += 1;
        }

        let addr = parts
            .iter()
            .rev()
            .find(|part| is_tracert_address_token(part))
            .map(|part| part.trim_matches(['[', ']']).to_string())
            .unwrap_or_else(|| "*".to_string());

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

fn is_tracert_address_token(token: &str) -> bool {
    let token = token.trim_matches(['[', ']']);
    if token.is_empty()
        || token == "*"
        || token == "<1"
        || token.eq_ignore_ascii_case("ms")
        || token.ends_with("ms")
        || token.parse::<f32>().is_ok()
    {
        return false;
    }

    token.parse::<std::net::IpAddr>().is_ok() || token.contains('.')
}

fn trace_route_tracepath(host: &str) -> Result<Vec<Hop>, String> {
    let output = crate::process::command("tracepath")
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
            let addr = if addr_raw == "no reply"
                || addr_raw.starts_with("LOCAL")
                || addr_raw.starts_with('[')
            {
                "*".to_string()
            } else {
                addr_raw.to_string()
            };

            let latency_ms = caps
                .get(3)
                .and_then(|m| m.as_str().trim_end_matches("ms").parse::<f32>().ok())
                .unwrap_or(0.0);
            let has_reply = latency_ms > 0.0;

            let hostname = resolve_hostname(&addr);

            hops.push(Hop {
                hop_number: hop_num,
                address: addr,
                hostname,
                avg_ms: latency_ms,
                min_ms: latency_ms,
                max_ms: latency_ms,
                loss_pct: if has_reply { 0.0 } else { 100.0 },
                status: classify_hop_status(
                    latency_ms,
                    if has_reply { 0.0 } else { 100.0 },
                    has_reply,
                ),
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
pub fn ping_continuous_parse_line(line: &str) -> Option<f32> {
    let line = line.trim();

    fn parse_after_marker(line: &str, marker: &str) -> Option<f32> {
        let pos = line.find(marker)?;
        let rest = &line[pos + marker.len()..];
        let end = rest
            .find(|c: char| c == ' ' || c == 'm' || (!c.is_ascii_digit() && c != '.'))
            .unwrap_or(rest.len());
        let val = rest[..end].trim();
        if val.is_empty() {
            None
        } else if marker.ends_with('<') && val == "1" {
            Some(0.5)
        } else {
            val.parse::<f32>().ok()
        }
    }

    for marker in ["time=", "time<", "tempo=", "tempo<"] {
        if let Some(ms) = parse_after_marker(line, marker) {
            return Some(ms);
        }
    }

    // Linux: time 12.3 ms (some variants)
    if let Some(pos) = line.find("time ") {
        let rest = &line[pos + 5..];
        let end = rest
            .find(|c: char| c == ' ' || c == 'm')
            .unwrap_or(rest.len());
        return rest[..end].parse::<f32>().ok();
    }

    // Any line with "bytes from" or "bytes=" and a number followed by "ms"
    if line.contains("bytes from") || line.contains("bytes=") || line.contains("icmp_seq") {
        for word in line.split_whitespace() {
            if word.ends_with("ms") {
                let num = word.trim_end_matches("ms");
                if num == "<1" {
                    return Some(0.5);
                }
                if let Ok(v) = num.parse::<f32>() {
                    return Some(v);
                }
            }
        }
    }
    None
}

#[allow(dead_code)]
pub fn classify_latency(ms: f32) -> &'static str {
    if ms < 5.0 {
        "Excelente"
    } else if ms < 30.0 {
        "Bom"
    } else if ms < 80.0 {
        "Aceitável"
    } else {
        "Ruim"
    }
}
