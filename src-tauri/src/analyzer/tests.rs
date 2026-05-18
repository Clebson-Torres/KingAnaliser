#[cfg(test)]
mod tests {
    use crate::analyzer::route::{classify_latency, parse_ping_output, parse_tracepath_output};
    use crate::analyzer::iface_stats::parse_proc_net_dev;

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
        assert_eq!(hops[0].latency_ms, "1.234ms");
        assert_eq!(hops[1].address, "10.198.0.79");
        assert_eq!(hops[2].address, "*");
        assert_eq!(hops[2].latency_ms, "?");
        assert_eq!(hops[3].hop_number, 4);
    }

    #[test]
    fn test_parse_tracepath_empty() {
        let result = parse_tracepath_output("");
        assert!(result.is_err());
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
        assert_eq!(result.transmitted, 4);
        assert_eq!(result.received, 4);
        assert_eq!(result.loss_pct, 0);
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
        assert_eq!(result.transmitted, 4);
        assert_eq!(result.loss_pct, 0);
        assert!((result.avg_ms - 15.100).abs() < 0.01);
    }

    #[test]
    fn test_parse_ping_loss() {
        let output = "PING 10.0.0.1 (10.0.0.1) 56(84) bytes of data.

--- 10.0.0.1 ping statistics ---
4 packets transmitted, 0 received, 100% packet loss, time 3004ms";
        let result = parse_ping_output(output, 4).unwrap();
        assert_eq!(result.received, 0);
        assert_eq!(result.loss_pct, 100);
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
