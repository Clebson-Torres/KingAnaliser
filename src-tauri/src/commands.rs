use crate::analyzer::{dns, dns_bench, gateway, http_timing, iface_stats, ip, mtr, ports, quality, route, report};

#[tauri::command]
pub async fn get_local_ip() -> Result<Vec<ip::InterfaceInfo>, String> {
    tokio::task::spawn_blocking(ip::get_network_interfaces)
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
}

#[tauri::command]
pub async fn get_network_interfaces() -> Result<Vec<ip::InterfaceInfo>, String> {
    tokio::task::spawn_blocking(ip::get_network_interfaces)
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
}

#[tauri::command]
pub async fn get_public_ip() -> Result<String, String> {
    tokio::task::spawn_blocking(ip::get_public_ip_address)
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
}

#[tauri::command]
pub async fn ping(host: String) -> Result<route::PingResult, String> {
    tokio::task::spawn_blocking(move || route::ping_host(&host, 4))
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
}

#[tauri::command]
pub async fn trace_route(host: String) -> Result<Vec<route::Hop>, String> {
    tokio::task::spawn_blocking(move || route::trace_route(&host))
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
}

#[tauri::command]
pub async fn get_listening_ports() -> Result<Vec<ports::ListeningPort>, String> {
    tokio::task::spawn_blocking(ports::get_listening_ports)
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
}

#[tauri::command]
pub async fn scan_ports(host: String, ports_list: Vec<u16>) -> Vec<ports::ScanResult> {
    tokio::task::spawn_blocking(move || ports::scan_ports(&host, &ports_list, 1500))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn get_port_list() -> Vec<u16> {
    ports::get_port_list()
}

#[tauri::command]
pub async fn dns_lookup(host: String) -> Result<dns::DnsResult, String> {
    tokio::task::spawn_blocking(move || dns::dns_lookup(&host))
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
}

#[tauri::command]
pub async fn get_gateway_info() -> Result<gateway::GatewayInfo, String> {
    tokio::task::spawn_blocking(gateway::get_gateway_info)
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
}

#[tauri::command]
pub async fn benchmark_dns() -> Vec<dns_bench::DnsServer> {
    tokio::task::spawn_blocking(dns_bench::benchmark_dns)
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn test_http_timing(urls: Vec<String>) -> Vec<http_timing::HttpTiming> {
    tokio::task::spawn_blocking(move || {
        urls.iter()
            .map(|url| http_timing::test_http_timing(url).unwrap_or_else(|_e| http_timing::HttpTiming {
                url: url.clone(),
                dns_ms: 0.0,
                connect_ms: 0.0,
                ttfb_ms: 0.0,
                total_ms: 0.0,
                status_code: 0,
                quality: "error".to_string(),
            }))
            .collect()
    })
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn get_http_targets() -> Vec<String> {
    http_timing::get_http_targets()
}

#[tauri::command]
pub async fn get_interface_stats() -> Result<Vec<iface_stats::IfaceStats>, String> {
    tokio::task::spawn_blocking(iface_stats::get_interface_stats)
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
}

#[tauri::command]
pub async fn run_mtr(host: String, cycles: u8) -> Result<Vec<mtr::MtrHop>, String> {
    tokio::task::spawn_blocking(move || mtr::run_mtr(&host, cycles))
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
}

#[tauri::command]
pub async fn get_quality_thresholds() -> quality::QualityThresholds {
    quality::get_thresholds()
}

#[tauri::command]
pub async fn generate_report(
    ip_local: String,
    ip_pub: String,
    dns: String,
    ping: String,
    traceroute: String,
    ports_str: String,
    scan: String,
    gateway: String,
    dns_bench: String,
    http_timing: String,
    iface_stats: String,
) -> String {
    report::generate_report(
        &ip_local, &ip_pub, &dns, &ping, &traceroute,
        &ports_str, &scan, &gateway, &dns_bench,
        &http_timing, &iface_stats,
    )
}
