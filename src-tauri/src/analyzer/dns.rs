use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DnsResult {
    pub host: String,
    pub addresses: Vec<String>,
    pub reverse: Option<String>,
}

pub fn dns_lookup(host: &str) -> Result<DnsResult, String> {
    use std::net::ToSocketAddrs;

    // Check if it's an IP or hostname
    let is_ip = host.parse::<std::net::IpAddr>().is_ok();

    if is_ip {
        let reverse = reverse_dns(host);
        return Ok(DnsResult {
            host: host.to_string(),
            addresses: vec![host.to_string()],
            reverse,
        });
    }

    let addr_str = format!("{}:0", host);
    let addrs: Vec<String> = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("Falha ao resolver DNS: {}", e))?
        .map(|a| a.ip().to_string())
        .collect();

    if addrs.is_empty() {
        return Err(format!("Nenhum endereço encontrado para {}", host));
    }

    let reverse = None;

    Ok(DnsResult {
        host: host.to_string(),
        addresses: addrs,
        reverse,
    })
}

fn reverse_dns(ip: &str) -> Option<String> {
    let output = if cfg!(target_os = "windows") {
        crate::process::command("nslookup")
            .args([ip])
            .output()
            .ok()?
    } else {
        crate::process::command("host").args([ip]).output().ok()?
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if cfg!(target_os = "windows") {
            if line.contains("Name:") {
                if let Some(name) = line.split(':').nth(1) {
                    return Some(name.trim().trim_end_matches('.').to_string());
                }
            }
        } else {
            if let Some(pos) = line.find("domain name pointer") {
                let rest = &line[pos + "domain name pointer".len()..];
                return Some(rest.trim().trim_end_matches('.').to_string());
            }
        }
    }
    None
}
