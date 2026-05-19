use super::route;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct MtrHop {
    pub hop: u8,
    pub host: String,
    pub loss_pct: f32,
    pub avg_ms: f32,
    pub best_ms: f32,
    pub worst_ms: f32,
    pub jitter_ms: f32,
    pub quality: String,
}

pub fn run_mtr(host: &str, cycles: u8) -> Result<Vec<MtrHop>, String> {
    if cfg!(target_os = "windows") {
        return run_mtr_windows(host, cycles);
    }

    let mtr_check = crate::process::command("which").arg("mtr").output();
    let mtr_available = mtr_check.map(|o| o.status.success()).unwrap_or(false);

    if mtr_available {
        let cycles_str = cycles.to_string();
        match crate::process::command("mtr")
            .args(["--report", "--report-cycles", &cycles_str, "--no-dns", host])
            .output()
        {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Ok(hops) = parse_mtr_output(&stdout) {
                    return Ok(hops);
                }
            }
            _ => {}
        }
    }

    // Fallback: use traceroute data as MTR-like results
    let hops = route::trace_route(host)?;
    Ok(hops
        .into_iter()
        .map(|h| MtrHop {
            hop: h.hop_number as u8,
            host: h.address,
            loss_pct: h.loss_pct,
            avg_ms: h.avg_ms,
            best_ms: h.min_ms,
            worst_ms: h.max_ms,
            jitter_ms: if h.max_ms > h.min_ms {
                h.max_ms - h.min_ms
            } else {
                0.0
            },
            quality: h.status,
        })
        .collect())
}

pub(crate) fn parse_mtr_output(output: &str) -> Result<Vec<MtrHop>, String> {
    let mut hops = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 8 {
            continue;
        }

        let hop_token: String = parts[0]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let hop: u8 = hop_token.parse().unwrap_or(0);
        if hop == 0 {
            continue;
        }

        let Some(loss_idx) = parts.iter().position(|p| p.ends_with('%')) else {
            continue;
        };
        if loss_idx < 2 || parts.len() <= loss_idx + 6 {
            continue;
        }

        let host = parts[1..loss_idx].join(" ");
        let loss_pct: f32 = parts[loss_idx].trim_end_matches('%').parse().unwrap_or(0.0);
        let avg_ms: f32 = parts[loss_idx + 3].parse().unwrap_or(0.0);
        let best_ms: f32 = parts[loss_idx + 4].parse().unwrap_or(0.0);
        let worst_ms: f32 = parts[loss_idx + 5].parse().unwrap_or(0.0);
        let jitter_ms: f32 = parts[loss_idx + 6].parse().unwrap_or(0.0);

        let quality = if avg_ms < 30.0 && loss_pct == 0.0 {
            "ok"
        } else if avg_ms < 80.0 || loss_pct <= 2.0 {
            "warning"
        } else {
            "critical"
        };

        hops.push(MtrHop {
            hop,
            host: host.to_string(),
            loss_pct,
            avg_ms,
            best_ms,
            worst_ms,
            jitter_ms,
            quality: quality.to_string(),
        });
    }

    if hops.is_empty() {
        Err("Nenhum hop encontrado no MTR".to_string())
    } else {
        Ok(hops)
    }
}

fn run_mtr_windows(host: &str, cycles: u8) -> Result<Vec<MtrHop>, String> {
    let hops = route::trace_route(host)?;
    if hops.is_empty() {
        return Err("Nenhum hop encontrado no Windows MTR".to_string());
    }

    Ok(hops
        .into_iter()
        .map(|hop| {
            if hop.address == "*" || hop.status == "no_reply" {
                return MtrHop {
                    hop: hop.hop_number as u8,
                    host: "*".to_string(),
                    loss_pct: 100.0,
                    avg_ms: 0.0,
                    best_ms: 0.0,
                    worst_ms: 0.0,
                    jitter_ms: 0.0,
                    quality: "critical".to_string(),
                };
            }

            match route::ping_host(&hop.address, cycles.max(1)) {
                Ok(ping) => MtrHop {
                    hop: hop.hop_number as u8,
                    host: hop.address,
                    loss_pct: ping.loss_pct,
                    avg_ms: ping.avg_ms,
                    best_ms: ping.min_ms,
                    worst_ms: ping.max_ms,
                    jitter_ms: ping.jitter_ms,
                    quality: if ping.quality_color == "red" {
                        "critical".to_string()
                    } else if ping.quality_color == "yellow" {
                        "warning".to_string()
                    } else {
                        "ok".to_string()
                    },
                },
                Err(_) => MtrHop {
                    hop: hop.hop_number as u8,
                    host: hop.address,
                    loss_pct: hop.loss_pct,
                    avg_ms: hop.avg_ms,
                    best_ms: hop.min_ms,
                    worst_ms: hop.max_ms,
                    jitter_ms: if hop.max_ms > hop.min_ms {
                        hop.max_ms - hop.min_ms
                    } else {
                        0.0
                    },
                    quality: hop.status,
                },
            }
        })
        .collect())
}
