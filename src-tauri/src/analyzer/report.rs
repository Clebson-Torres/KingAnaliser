use crate::analyzer::quality;

pub fn generate_report(
    ip_local: &str,
    ip_pub: &str,
    dns_info: &str,
    ping: &str,
    traceroute: &str,
    ports_str: &str,
    scan: &str,
    gateway: &str,
    dns_bench: &str,
    http_timing: &str,
    iface_stats: &str,
) -> String {
    let now = chrono::Local::now().format("%d/%m/%Y %H:%M:%S").to_string();
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "(desconhecido)".to_string());

    let mut r = String::new();

    r.push_str(&format!("{}\n", "=".repeat(58)));
    r.push_str("  KING ANALISER — RELATÓRIO DE ANÁLISE DE REDE\n");
    r.push_str(&format!("  Data: {}  |  Host: {}\n", now, hostname));
    r.push_str(&format!("{}\n\n", "=".repeat(58)));

    let general_quality = extract_general_quality(ping, traceroute);
    let worst_hop = extract_worst_hop(traceroute);
    let problem_text = if let Some(hop) = worst_hop {
        format!("Salto {} com latência elevada", hop)
    } else if ping.contains("Perda") || ping.contains("perda") || ping.contains("loss") {
        "Perda de pacotes detectada".to_string()
    } else {
        "Nenhum problema detectado".to_string()
    };

    r.push_str(&format!("{}\n", "═".repeat(50)));
    r.push_str("  RESUMO EXECUTIVO\n");
    r.push_str(&format!("{}\n", "═".repeat(50)));
    r.push_str(&format!("  Qualidade geral:  {}\n", general_quality));
    r.push_str(&format!("  Problema:         {}\n", problem_text));
    r.push_str("\n");

    r.push_str(&format!("--- [1] Interfaces de Rede ---\n"));
    r.push_str(ip_local);
    r.push_str("\n");

    r.push_str(&format!("--- [2] Gateway ---\n"));
    r.push_str(gateway);
    r.push_str("\n");

    r.push_str(&format!("--- [3] IP Público ---\n"));
    r.push_str(ip_pub);
    r.push_str("\n");

    r.push_str(&format!("--- [4] DNS Benchmark ---\n"));
    r.push_str(dns_bench);
    r.push_str("\n");

    r.push_str(&format!("--- [5] Latência / Ping ---\n"));
    r.push_str(ping);
    r.push_str("\n");

    r.push_str(&format!("--- [6] Rota / Traceroute ---\n"));
    r.push_str(traceroute);
    r.push_str("\n");

    r.push_str(&format!("--- [7] Portas em Escuta ---\n"));
    r.push_str(ports_str);
    r.push_str("\n");

    r.push_str(&format!("--- [8] Scan TCP ---\n"));
    r.push_str(scan);
    r.push_str("\n");

    r.push_str(&format!("--- [9] Tempo HTTP ---\n"));
    r.push_str(http_timing);
    r.push_str("\n");

    r.push_str(&format!("--- [10] Estatísticas de Interface ---\n"));
    r.push_str(iface_stats);
    r.push_str("\n");

    r.push_str(&format!("--- [11] DNS Lookup ---\n"));
    r.push_str(dns_info);
    r.push_str("\n");

    r.push_str(&format!("\n{}\n", "═".repeat(50)));
    r.push_str(&format!("Relatório gerado em {}\n", now));
    r.push_str(&format!("{}\n", "═".repeat(50)));

    r
}

fn extract_general_quality(ping: &str, _traceroute: &str) -> String {
    let mut avg_ms = 999.0f32;
    for line in ping.lines() {
        if line.contains("Médio") || line.contains("Média") || line.contains("méd") || line.contains("avg") || line.contains("mdev") {
            if let Some(val) = line.split('/').nth(1) {
                let cleaned: String = val.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
                if let Ok(v) = cleaned.parse::<f32>() {
                    avg_ms = v;
                }
            }
        }
    }
    if avg_ms == 999.0 {
        for line in ping.lines() {
            if line.contains("Perda") || line.contains("perda") || line.contains("loss") {
                if line.contains("100%") {
                    return "Ruim".to_string();
                }
            }
        }
    }
    quality::classify_latency(avg_ms).to_string()
}

fn extract_worst_hop(traceroute: &str) -> Option<String> {
    for line in traceroute.lines() {
        if line.contains("ms") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for part in parts.iter() {
                if let Some(ms) = part.trim_end_matches("ms").parse::<f32>().ok() {
                    if ms > 100.0 {
                        let hop = parts.iter().find(|p| p.ends_with('.') || p.parse::<u32>().is_ok());
                        if let Some(h) = hop {
                            return Some(h.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}
