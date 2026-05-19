#[cfg(test)]
mod tests {
    use crate::analyzer::iface_stats::parse_proc_net_dev;
    use crate::analyzer::mtr::parse_mtr_output;
    use crate::analyzer::network_scan::detect_subnet;
    use crate::analyzer::report::generate_report;
    use crate::analyzer::route::{
        classify_latency, parse_ping_output, parse_tracepath_output, parse_tracert_output,
        ping_continuous_parse_line,
    };

    #[test]
    fn test_parse_tracepath_normal() {
        let output = " 1?: [LOCALHOST]                      pmtu 1500
 1:  192.168.1.1                                           1.234ms
 2:  10.198.0.79                                          47.123ms
 3:  no reply
 4:  72.14.237.129                                        12.5ms";
        let hops = parse_tracepath_output(output).unwrap();
        assert_eq!(hops.len(), 4);
        assert_eq!(hops[0].hop_number, 1);
        assert_eq!(hops[0].address, "192.168.1.1");
        assert!((hops[0].avg_ms - 1.234).abs() < 0.01);
        assert_eq!(hops[0].status, "ok");
        assert_eq!(hops[1].address, "10.198.0.79");
        assert_eq!(hops[2].address, "*");
        assert_eq!(hops[2].loss_pct, 100.0);
        assert_eq!(hops[2].status, "no_reply");
        assert_eq!(hops[3].hop_number, 4);
    }

    #[test]
    fn test_parse_tracepath_empty() {
        let result = parse_tracepath_output("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_windows_tracert() {
        let output = "Tracing route to 8.8.8.8 over a maximum of 30 hops

  1    <1 ms    <1 ms    <1 ms  192.168.1.1
  2    10 ms    11 ms    12 ms  10.0.0.1
  3     *        *        *     Request timed out.
  4    20 ms    21 ms    22 ms  8.8.8.8";

        let hops = parse_tracert_output(output).unwrap();
        assert_eq!(hops.len(), 4);
        assert_eq!(hops[0].address, "192.168.1.1");
        assert!((hops[0].avg_ms - 0.5).abs() < 0.01);
        assert_eq!(hops[1].address, "10.0.0.1");
        assert!((hops[1].avg_ms - 11.0).abs() < 0.01);
        assert_eq!(hops[2].status, "no_reply");
        assert_eq!(hops[3].address, "8.8.8.8");
    }

    #[test]
    fn test_parse_ping_en() {
        let output = "PING 8.8.8.8 (8.8.8.8) 56(84) bytes of data.
64 bytes from 8.8.8.8: icmp_seq=1 ttl=118 time=12.3 ms
64 bytes from 8.8.8.8: icmp_seq=2 ttl=118 time=11.8 ms
64 bytes from 8.8.8.8: icmp_seq=3 ttl=118 time=12.1 ms
64 bytes from 8.8.8.8: icmp_seq=4 ttl=118 time=11.9 ms

--- 8.8.8.8 ping statistics ---
4 packets transmitted, 4 received, 0% packet loss, time 3004ms
        rtt min/avg/max/mdev = 11.800/12.025/12.300/0.183 ms";
        let result = parse_ping_output(output, 4).unwrap();
        assert_eq!(result.packets_sent, 4);
        assert_eq!(result.packets_received, 4);
        assert_eq!(result.loss_pct, 0.0);
        assert!((result.avg_ms - 12.025).abs() < 0.01);
        assert!((result.min_ms - 11.800).abs() < 0.01);
        assert!((result.max_ms - 12.300).abs() < 0.01);
    }

    #[test]
    fn test_parse_ping_ptbr() {
        let output = "PING 8.8.8.8 (8.8.8.8) 56(84) bytes of data.
64 bytes from 8.8.8.8: icmp_seq=1 ttl=118 time=15.2 ms
64 bytes from 8.8.8.8: icmp_seq=2 ttl=118 time=14.8 ms
64 bytes from 8.8.8.8: icmp_seq=3 ttl=118 time=15.5 ms
64 bytes from 8.8.8.8: icmp_seq=4 ttl=118 time=14.9 ms

--- 8.8.8.8 ping statistics ---
4 pacotes transmitidos, 4 recebidos, 0% perda de pacotes, tempo 3004ms
        rtt min/avg/max/mdev = 14.800/15.100/15.500/0.260 ms";
        let result = parse_ping_output(output, 4).unwrap();
        assert_eq!(result.packets_sent, 4);
        assert_eq!(result.loss_pct, 0.0);
        assert!((result.avg_ms - 15.100).abs() < 0.01);
    }

    #[test]
    fn test_parse_ping_loss() {
        let output = "PING 10.0.0.1 (10.0.0.1) 56(84) bytes of data.

        --- 10.0.0.1 ping statistics ---
4 packets transmitted, 0 received, 100% packet loss, time 3004ms";
        let result = parse_ping_output(output, 4).unwrap();
        assert_eq!(result.packets_received, 0);
        assert_eq!(result.loss_pct, 100.0);
    }

    #[test]
    fn test_parse_windows_ping_ptbr() {
        let output = "Disparando 8.8.8.8 com 32 bytes de dados:
Resposta de 8.8.8.8: bytes=32 tempo=2ms TTL=118
Resposta de 8.8.8.8: bytes=32 tempo=3ms TTL=118
Resposta de 8.8.8.8: bytes=32 tempo<1ms TTL=118

Estatisticas do Ping para 8.8.8.8:
    Pacotes: Enviados = 3, Recebidos = 3, Perdidos = 0 (0% de perda),";

        let result = parse_ping_output(output, 3).unwrap();
        assert_eq!(result.host, "8.8.8.8");
        assert_eq!(result.packets_sent, 3);
        assert_eq!(result.packets_received, 3);
        assert_eq!(result.loss_pct, 0.0);
        assert!((result.min_ms - 0.5).abs() < 0.01);
        assert!((result.avg_ms - 1.833).abs() < 0.01);
        assert!((result.max_ms - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_report_summary_does_not_flag_zero_loss() {
        let ping = "Host: 8.8.8.8
+----------+-----------+----------+-------+--------+--------+--------+--------+-----------+
| Enviados | Recebidos | Perdidos | Perda | Min    | Media  | Max    | Jitter | Qualidade |
+----------+-----------+----------+-------+--------+--------+--------+--------+-----------+
| 10       | 10        | 0        | 0.0%  | 1.0 ms | 2.0 ms | 3.0 ms | 1.0 ms | Excelente |
+----------+-----------+----------+-------+--------+--------+--------+--------+-----------+";

        let report = generate_report(
            "interfaces",
            "ip publico",
            "dns",
            ping,
            "Destino: 8.8.8.8\nResumo: 1 hops, 1 com resposta, 0 sem resposta",
            "portas",
            "scan",
            "gateway",
            "| Servidor | IP | Latencia | Status | Melhor |\n| Google DNS | 8.8.8.8 | 20 ms | OK | sim |",
            "| URL | Status | Connect | TTFB | Total | Qualidade |\n| https://github.com | 200 | 10 ms | 20 ms | 30 ms | ok |",
            "iface",
            "inicio",
            "fim",
        );

        assert!(report.contains("Qualidade geral: Excelente"));
        assert!(report.contains("Diagnostico rapido: Nenhum problema detectado"));
        assert!(!report.contains("Perda de pacotes detectada"));
    }

    #[test]
    fn test_ping_continuous_parse_variants() {
        assert_eq!(
            ping_continuous_parse_line("64 bytes from 8.8.8.8: icmp_seq=1 ttl=118 time=12.3 ms"),
            Some(12.3)
        );
        assert_eq!(
            ping_continuous_parse_line("Resposta de 8.8.8.8: bytes=32 tempo=7ms TTL=118"),
            Some(7.0)
        );
        assert_eq!(
            ping_continuous_parse_line("Resposta de 127.0.0.1: bytes=32 tempo<1ms TTL=128"),
            Some(0.5)
        );
        assert_eq!(
            ping_continuous_parse_line("64 bytes from 127.0.0.1: icmp_seq=1 ttl=64 time<1 ms"),
            Some(0.5)
        );
    }

    #[test]
    fn test_detect_subnet_keeps_three_octets() {
        let (base, prefix) = detect_subnet(Some("192.168.1.0/24".to_string())).unwrap();
        assert_eq!(base, "192.168.1");
        assert_eq!(prefix, 24);

        let (base, prefix) = detect_subnet(Some("10.0.42.99".to_string())).unwrap();
        assert_eq!(base, "10.0.42");
        assert_eq!(prefix, 24);
    }

    #[test]
    fn test_detect_subnet_rejects_invalid_input() {
        assert!(detect_subnet(Some("192.168.x.0/24".to_string())).is_err());
        assert!(detect_subnet(Some("192.168.1.0/not-a-prefix".to_string())).is_err());
    }

    #[test]
    fn test_parse_mtr_report_output() {
        let output =
            "HOST: local                         Loss%   Snt   Last   Avg  Best  Wrst StDev
  1.|-- 172.18.1.10                  0.0%     5    0.8   0.9   0.6   1.4   0.3
  2.|-- 8.8.8.8                      0.0%     5   12.0  12.4  11.8  13.1   0.5";

        let hops = parse_mtr_output(output).unwrap();
        assert_eq!(hops.len(), 2);
        assert_eq!(hops[0].hop, 1);
        assert_eq!(hops[0].host, "172.18.1.10");
        assert!((hops[0].avg_ms - 0.9).abs() < 0.01);
        assert!((hops[0].best_ms - 0.6).abs() < 0.01);
        assert!((hops[0].worst_ms - 1.4).abs() < 0.01);
        assert!((hops[0].jitter_ms - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_classify_latency() {
        assert_eq!(classify_latency(0.0), "Excelente");
        assert_eq!(classify_latency(2.5), "Excelente");
        assert_eq!(classify_latency(4.9), "Excelente");
        assert_eq!(classify_latency(5.0), "Bom");
        assert_eq!(classify_latency(15.0), "Bom");
        assert_eq!(classify_latency(29.9), "Bom");
        assert_eq!(classify_latency(30.0), "Aceitável");
        assert_eq!(classify_latency(50.0), "Aceitável");
        assert_eq!(classify_latency(79.9), "Aceitável");
        assert_eq!(classify_latency(80.0), "Ruim");
        assert_eq!(classify_latency(500.0), "Ruim");
    }

    #[test]
    fn test_parse_proc_net_dev() {
        let content = "Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
  eth0: 1024000000   10000    0    0    0     0          0         0  512000000    8000    0    0    0     0       0          0
  wlan0:  204800000    2000    3    1    0     0          0         0  102400000    1500    2    0    0     0       0          0";
        let stats = parse_proc_net_dev(content).unwrap();
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].name, "eth0");
        assert!((stats[0].rx_mb - 976.5625).abs() < 0.01);
        assert!((stats[0].tx_mb - 488.28125).abs() < 0.01);
        assert_eq!(stats[0].rx_errors, 0);
        assert_eq!(stats[0].tx_errors, 0);
        assert_eq!(stats[1].name, "wlan0");
        assert_eq!(stats[1].rx_errors, 3);
        assert_eq!(stats[1].tx_errors, 2);
        assert_eq!(stats[1].rx_dropped, 1);
    }
}
