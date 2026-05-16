use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HttpTiming {
    pub url: String,
    pub dns_s: f32,
    pub connect_s: f32,
    pub ttfb_s: f32,
    pub total_s: f32,
    pub status_code: u16,
}

const TARGETS: &[&str] = &[
    "https://google.com",
    "https://cloudflare.com",
    "https://github.com",
];

pub fn get_http_targets() -> Vec<String> {
    TARGETS.iter().map(|s| s.to_string()).collect()
}

pub fn test_http_timing(url: &str) -> Result<HttpTiming, String> {
    let format_str = "%{time_namelookup}\\n%{time_connect}\\n%{time_starttransfer}\\n%{time_total}\\n%{http_code}";
    let output = std::process::Command::new("curl")
        .args([
            "-o", "/dev/null",
            "-s",
            "-w", format_str,
            url,
            "--max-time", "15",
        ])
        .output()
        .map_err(|e| format!("Falha ao executar curl: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("curl falhou para {}: {}", url, stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    if lines.len() < 5 {
        return Err(format!("Resposta inesperada do curl: {}", stdout));
    }

    let dns_s = lines[0].parse::<f32>().unwrap_or(0.0);
    let connect_s = lines[1].parse::<f32>().unwrap_or(0.0);
    let ttfb_s = lines[2].parse::<f32>().unwrap_or(0.0);
    let total_s = lines[3].parse::<f32>().unwrap_or(0.0);
    let status_code = lines[4].parse::<u16>().unwrap_or(0);

    Ok(HttpTiming {
        url: url.to_string(),
        dns_s,
        connect_s,
        ttfb_s,
        total_s,
        status_code,
    })
}
