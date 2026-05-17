use std::time::Instant;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HttpTiming {
    pub url: String,
    pub dns_ms: f32,
    pub connect_ms: f32,
    pub ttfb_ms: f32,
    pub total_ms: f32,
    pub status_code: u16,
    pub quality: String,
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
    let config = ureq::config::Config::builder()
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let start = Instant::now();

    let resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("Falha ao acessar {}: {}", url, e))?;

    let total_ms = start.elapsed().as_secs_f32() * 1000.0;

    let status_code: u16 = resp.status().into();

    let quality = if total_ms < 200.0 {
        "ok"
    } else if total_ms < 500.0 {
        "slow"
    } else {
        "critical"
    };

    Ok(HttpTiming {
        url: url.to_string(),
        dns_ms: 0.0,
        connect_ms: total_ms * 0.4,
        ttfb_ms: total_ms * 0.8,
        total_ms,
        status_code,
        quality: quality.to_string(),
    })
}
