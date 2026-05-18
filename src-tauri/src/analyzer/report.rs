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

    r.push_str("KINGNETWORKTOOLS - RELATORIO DE DIAGNOSTICO\n");
    r.push_str("============================================================\n");
    r.push_str(&format!("Data: {}\n", now));
    r.push_str(&format!("Host local: {}\n", hostname));
    r.push_str("============================================================\n\n");

    r.push_str("RESUMO EXECUTIVO\n");
    r.push_str("------------------------------------------------------------\n");

    let general_quality = extract_general_quality(ping);
    let worst_hop = extract_worst_hop(traceroute);
    let problem_text = if let Some(hop) = worst_hop {
        format!("Salto {} com latência elevada", hop)
    } else if ping.contains("Perda")
        || ping.contains("perda")
        || ping.contains("loss")
        || ping.contains("packet loss")
    {
        "Perda de pacotes detectada".to_string()
    } else {
        "Nenhum problema detectado".to_string()
    };

    r.push_str(&format!("Qualidade geral: {}\n", general_quality));
    r.push_str(&format!("Diagnostico rapido: {}\n", problem_text));
    r.push_str("Leitura sugerida: verifique primeiro Gateway, DNS, Ping e Rota; depois Portas/HTTP para sintomas especificos.\n");
    r.push_str("\n");

    push_section(&mut r, "1. Interfaces de rede", ip_local);
    push_section(&mut r, "2. Gateways padrao", gateway);
    push_section(&mut r, "3. IP publico e geolocalizacao", ip_pub);
    push_section(&mut r, "4. DNS lookup do alvo", dns_info);
    push_section(&mut r, "5. Benchmark DNS", dns_bench);
    push_section(&mut r, "6. Ping do alvo", ping);
    push_section(&mut r, "7. Rota / Traceroute", traceroute);
    push_section(&mut r, "8. Portas em escuta local", ports_str);
    push_section(&mut r, "9. Scan TCP do alvo", scan);
    push_section(&mut r, "10. Tempo HTTP", http_timing);
    push_section(&mut r, "11. Estatisticas de interface", iface_stats);

    r.push_str("FIM DO RELATORIO\n");
    r.push_str("------------------------------------------------------------\n");
    r.push_str(&format!("Gerado por KingNetworkTools em {}\n", now));

    r
}

fn push_section(report: &mut String, title: &str, body: &str) {
    report.push_str(title);
    report.push('\n');
    report.push_str("------------------------------------------------------------\n");
    report.push_str(body.trim());
    report.push_str("\n\n");
}

fn extract_general_quality(ping: &str) -> String {
    let mut avg_ms = 999.0f32;
    for line in ping.lines() {
        if line.contains("Médio")
            || line.contains("Média")
            || line.contains("méd")
            || line.contains("avg")
            || line.contains("mdev")
        {
            if let Some(val) = line.split('/').nth(1) {
                let cleaned: String = val
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
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
                        let hop = parts
                            .iter()
                            .find(|p| p.ends_with('.') || p.parse::<u32>().is_ok());
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
