use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct IfaceStats {
    pub name: String,
    pub rx_mb: f64,
    pub tx_mb: f64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
}

pub fn get_interface_stats() -> Result<Vec<IfaceStats>, String> {
    if cfg!(target_os = "windows") {
        return get_stats_windows();
    }
    get_stats_linux()
}

fn get_stats_linux() -> Result<Vec<IfaceStats>, String> {
    let content = std::fs::read_to_string("/proc/net/dev")
        .map_err(|e| format!("Falha ao ler /proc/net/dev: {}", e))?;

    parse_proc_net_dev(&content)
}

pub(crate) fn parse_proc_net_dev(content: &str) -> Result<Vec<IfaceStats>, String> {
    let mut stats = Vec::new();

    for line in content.lines().skip(2) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }

        let name = parts[0].trim_end_matches(':').to_string();
        if name.starts_with("lo") {
            continue;
        }

        let rx_bytes = parts[1].parse::<f64>().unwrap_or(0.0);
        let tx_bytes = parts[9].parse::<f64>().unwrap_or(0.0);
        let rx_errors = parts[3].parse::<u64>().unwrap_or(0);
        let tx_errors = parts[11].parse::<u64>().unwrap_or(0);
        let rx_dropped = parts[4].parse::<u64>().unwrap_or(0);

        stats.push(IfaceStats {
            name,
            rx_mb: rx_bytes / (1024.0 * 1024.0),
            tx_mb: tx_bytes / (1024.0 * 1024.0),
            rx_errors,
            tx_errors,
            rx_dropped,
        });
    }

    if stats.is_empty() {
        Err("Nenhuma estatística de interface encontrada".to_string())
    } else {
        Ok(stats)
    }
}

fn get_stats_windows() -> Result<Vec<IfaceStats>, String> {
    let output = crate::process::command("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-NetAdapterStatistics | Select-Object Name, ReceivedBytes, SentBytes, ReceivedUnicastPackets, SentUnicastPackets | ConvertTo-Json",
        ])
        .output()
        .map_err(|e| format!("Falha ao executar Get-NetAdapterStatistics: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
        let mut stats = Vec::new();
        for iface in parsed {
            let name = iface["Name"].as_str().unwrap_or("unknown").to_string();
            let rx_bytes = iface["ReceivedBytes"].as_f64().unwrap_or(0.0);
            let tx_bytes = iface["SentBytes"].as_f64().unwrap_or(0.0);

            stats.push(IfaceStats {
                name,
                rx_mb: rx_bytes / (1024.0 * 1024.0),
                tx_mb: tx_bytes / (1024.0 * 1024.0),
                rx_errors: 0,
                tx_errors: 0,
                rx_dropped: 0,
            });
        }
        if stats.is_empty() {
            Err("Nenhuma estatística encontrada no Windows".to_string())
        } else {
            Ok(stats)
        }
    } else {
        if let Ok(single) = serde_json::from_str::<serde_json::Value>(&stdout) {
            let name = single["Name"].as_str().unwrap_or("unknown").to_string();
            let rx_bytes = single["ReceivedBytes"].as_f64().unwrap_or(0.0);
            let tx_bytes = single["SentBytes"].as_f64().unwrap_or(0.0);
            return Ok(vec![IfaceStats {
                name,
                rx_mb: rx_bytes / (1024.0 * 1024.0),
                tx_mb: tx_bytes / (1024.0 * 1024.0),
                rx_errors: 0,
                tx_errors: 0,
                rx_dropped: 0,
            }]);
        }
        Err("Falha ao parsear estatísticas do Windows".to_string())
    }
}
