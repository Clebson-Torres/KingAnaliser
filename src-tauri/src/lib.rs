mod analyzer;
mod commands;
mod process;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_local_ip,
            commands::get_network_interfaces,
            commands::get_public_ip,
            commands::get_public_ip_info,
            commands::ping,
            commands::trace_route,
            commands::get_listening_ports,
            commands::scan_ports,
            commands::get_port_list,
            commands::dns_lookup,
            commands::get_gateway_info,
            commands::benchmark_dns,
            commands::test_http_timing,
            commands::get_http_targets,
            commands::get_interface_stats,
            commands::run_mtr,
            commands::get_quality_thresholds,
            commands::scan_network,
            commands::generate_report,
            commands::start_continuous_ping,
        ])
        .run(tauri::generate_context!())
        .expect("Erro ao iniciar o aplicativo Tauri");
}
