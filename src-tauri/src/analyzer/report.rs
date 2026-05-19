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
    started_at: &str,
    ended_at: &str,
) -> String {
    let now = chrono::Local::now().format("%d/%m/%Y %H:%M:%S").to_string();
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "(desconhecido)".to_string());

    let mut r = String::new();

    r.push_str("KINGNETWORKTOOLS - RELATORIO DE DIAGNOSTICO\n");
    r.push_str("============================================================\n");
    r.push_str(&format!("Data: {}\n", now));
    r.push_str(&format!("Inicio da coleta: {}\n", started_at));
    r.push_str(&format!("Fim da coleta: {}\n", ended_at));
    r.push_str(&format!("Host local: {}\n", hostname));
    r.push_str("============================================================\n\n");

    r.push_str("RESUMO EXECUTIVO\n");
    r.push_str("------------------------------------------------------------\n");

    let ping_summary = extract_ping_summary(ping);
    let dns_slow = has_latency_above(dns_bench, 100.0);
    let http_slow = http_timing.contains("slow") || has_latency_above(http_timing, 250.0);
    let general_quality = extract_general_quality(&ping_summary, dns_slow || http_slow);
    let worst_hop = extract_worst_hop(traceroute);
    let problem_text = if let Some(hop) = worst_hop {
        format!("Salto {} com latência elevada", hop)
    } else if ping_summary.loss_pct.unwrap_or(0.0) > 0.0 {
        format!(
            "Perda de pacotes detectada ({:.1}%)",
            ping_summary.loss_pct.unwrap_or(0.0)
        )
    } else if dns_slow && http_slow {
        "DNS/HTTP com latência elevada em alguns destinos".to_string()
    } else if dns_slow {
        "DNS com latência elevada".to_string()
    } else if http_slow {
        "HTTP com latência elevada em alguns destinos".to_string()
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

#[derive(Default)]
struct PingSummary {
    avg_ms: Option<f32>,
    loss_pct: Option<f32>,
    quality: Option<String>,
}

fn extract_ping_summary(ping: &str) -> PingSummary {
    let mut summary = PingSummary::default();

    for line in ping.lines() {
        let cells: Vec<&str> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim())
            .collect();

        if cells.len() >= 9 && cells[0].parse::<u32>().is_ok() {
            summary.loss_pct = parse_percent(cells[3]);
            summary.avg_ms = parse_ms(cells[5]);
            summary.quality = Some(cells[8].to_string());
            return summary;
        }
    }

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
                    summary.avg_ms = Some(v);
                }
            }
        }

        if line.contains('%')
            && (line.contains("Perda") || line.contains("perda") || line.contains("loss"))
        {
            for part in line.split_whitespace() {
                if let Some(loss) = parse_percent(part) {
                    summary.loss_pct = Some(loss);
                    break;
                }
            }
        }
    }

    summary
}

fn extract_general_quality(ping: &PingSummary, has_secondary_latency: bool) -> String {
    if ping.loss_pct.unwrap_or(0.0) >= 20.0 {
        return "Ruim".to_string();
    }

    if let Some(quality) = &ping.quality {
        if has_secondary_latency && (quality == "Excelente" || quality == "Bom") {
            return "Aceitável".to_string();
        }
        return quality.clone();
    }

    let avg_ms = ping.avg_ms.unwrap_or(999.0);
    let quality = quality::classify_latency(avg_ms).to_string();
    if has_secondary_latency && (quality == "Excelente" || quality == "Bom") {
        "Aceitável".to_string()
    } else {
        quality
    }
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

fn has_latency_above(text: &str, threshold_ms: f32) -> bool {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|pair| pair[1] == "ms" && parse_number(pair[0]).map_or(false, |ms| ms > threshold_ms))
}

fn parse_ms(text: &str) -> Option<f32> {
    parse_number(text.trim_end_matches("ms").trim())
}

fn parse_percent(text: &str) -> Option<f32> {
    parse_number(text.trim_end_matches('%').trim())
}

fn parse_number(text: &str) -> Option<f32> {
    let cleaned: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect::<String>()
        .replace(',', ".");

    if cleaned.is_empty() {
        None
    } else {
        cleaned.parse::<f32>().ok()
    }
}
