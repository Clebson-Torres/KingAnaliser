use crate::analyzer::{
    dns, dns_bench, gateway, http_timing, iface_stats, ip, mtr, network_scan, ports, quality,
    report, route,
};
use tauri::Emitter;

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
pub async fn get_public_ip_info() -> Result<ip::IpInfo, String> {
    tokio::task::spawn_blocking(ip::get_public_ip_info)
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
}

#[derive(serde::Serialize, Clone)]
pub struct PingEventPayload {
    pub sequence: u32,
    pub latency_ms: f32,
    pub success: bool,
    pub host: String,
    pub done: bool,
    pub total: u32,
}

#[tauri::command]
pub async fn ping(host: String, count: Option<u8>) -> Result<route::PingResult, String> {
    let count = count.unwrap_or(10);
    tokio::task::spawn_blocking(move || route::ping_host(&host, count))
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
}

#[tauri::command]
pub async fn start_continuous_ping(
    app_handle: tauri::AppHandle,
    host: String,
    count: u32,
    interval_ms: u64,
) -> Result<(), String> {
    let host_clone = host.clone();
    let count_clone = count;

    tokio::task::spawn_blocking(move || {
        let mut sequence = 0u32;
        let mut remaining = count_clone;

        while remaining > 0 {
            let start = std::time::Instant::now();
            let cmd = if cfg!(target_os = "windows") {
                std::process::Command::new("ping")
                    .args(["-n", "1", "-w", "3000", &host_clone])
                    .output()
            } else {
                std::process::Command::new("ping")
                    .args(["-c", "1", "-W", "3", &host_clone])
                    .output()
            };

            let elapsed = start.elapsed().as_secs_f32() * 1000.0;
            sequence += 1;
            remaining -= 1;

            let (latency_ms, success) = match &cmd {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Some(ms) = route::ping_continuous_parse_line(&stdout) {
                        (ms, true)
                    } else {
                        (elapsed, false)
                    }
                }
                Ok(_) => {
                    let stdout = String::from_utf8_lossy(&cmd.as_ref().unwrap().stdout);
                    let ms = route::ping_continuous_parse_line(&stdout).unwrap_or(0.0);
                    (ms, ms > 0.0)
                }
                Err(_) => (0.0, false),
            };

            let done = remaining == 0;
            let payload = PingEventPayload {
                sequence,
                latency_ms,
                success,
                host: host_clone.clone(),
                done,
                total: count_clone,
            };

            let _ = app_handle.emit("ping-event", payload);

            if done {
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(interval_ms));
        }
    });

    Ok(())
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
pub async fn scan_ports(
    host: String,
    ports_list: Vec<u16>,
    timeout_ms: Option<u64>,
) -> Vec<ports::ScanResult> {
    let timeout = timeout_ms.unwrap_or(1500);
    tokio::task::spawn_blocking(move || ports::scan_ports(&host, &ports_list, timeout))
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
            .map(|url| {
                http_timing::test_http_timing(url).unwrap_or_else(|_e| http_timing::HttpTiming {
                    url: url.clone(),
                    dns_ms: 0.0,
                    connect_ms: 0.0,
                    ttfb_ms: 0.0,
                    total_ms: 0.0,
                    status_code: 0,
                    quality: "error".to_string(),
                })
            })
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
pub async fn scan_network(
    subnet: Option<String>,
) -> Result<network_scan::NetworkScanResult, String> {
    tokio::task::spawn_blocking(move || network_scan::scan_network(subnet))
        .await
        .map_err(|e| format!("Erro interno: {}", e))?
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
        &ip_local,
        &ip_pub,
        &dns,
        &ping,
        &traceroute,
        &ports_str,
        &scan,
        &gateway,
        &dns_bench,
        &http_timing,
        &iface_stats,
    )
}
