pub fn generate_report(
    ip_local: &str,
    ip_pub: &str,
    _dns: &str,
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
    let mut r = String::new();

    r.push_str("=== RELATÓRIO DE ANÁLISE DE REDE ===\n");
    r.push_str(&format!("Data: {}\n\n", now));

    r.push_str("--- [1] Interfaces de Rede ---\n");
    r.push_str(ip_local);
    r.push_str("\n");

    r.push_str("--- [2] Gateway ---\n");
    r.push_str(gateway);
    r.push_str("\n");

    r.push_str("--- [3] IP Público ---\n");
    r.push_str(ip_pub);
    r.push_str("\n");

    r.push_str("--- [4] DNS Benchmark ---\n");
    r.push_str(dns_bench);
    r.push_str("\n");

    r.push_str("--- [5] Latência / Ping ---\n");
    r.push_str(ping);
    r.push_str("\n");

    r.push_str("--- [6] Traceroute ---\n");
    r.push_str(traceroute);
    r.push_str("\n");

    r.push_str("--- [7] Portas em Escuta ---\n");
    r.push_str(ports_str);
    r.push_str("\n");

    r.push_str("--- [8] Scan TCP ---\n");
    r.push_str(scan);
    r.push_str("\n");

    r.push_str("--- [9] Tempo HTTP ---\n");
    r.push_str(http_timing);
    r.push_str("\n");

    r.push_str("--- [10] Estatísticas de Interface ---\n");
    r.push_str(iface_stats);
    r.push_str("\n");

    r.push_str(&"═".repeat(50));
    r.push_str("\n  RESUMO\n");
    r.push_str(&"═".repeat(50));
    r.push_str("\n");

    if !gateway.is_empty() {
        r.push_str(&format!("  Gateway: {}\n", gateway.lines().next().unwrap_or("")));
    }
    if !ping.is_empty() {
        for line in ping.lines() {
            if line.contains("Perda") || line.contains("perda") {
                r.push_str(&format!("  Ping: {}\n", line.trim()));
            }
        }
    }

    r.push_str(&format!("\nRelatório gerado em {}\n", now));
    r
}
