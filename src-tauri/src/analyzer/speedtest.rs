use serde::Serialize;
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::Emitter;

use super::route;

const NUM_THREADS: u32 = 4;
const WARMUP_SECS: f32 = 2.0;
const DOWNLOAD_SECS: f32 = 10.0;
const UPLOAD_SECS: f32 = 8.0;
const CHUNK_SIZE: u64 = 5_000_000;
const UPLOAD_CHUNK_SIZE: usize = 2_000_000;
const SERVER_NAME: &str = "Cloudflare (GRU)";

#[derive(Debug, Serialize, Clone)]
pub struct SpeedTestProgress {
    pub phase: String,
    pub progress_pct: u8,
    pub current_mbps: f32,
    pub server: String,
}

#[derive(Debug, Serialize)]
pub struct SpeedTestResult {
    pub download_mbps: f32,
    pub upload_mbps: f32,
    pub latency_ms: f32,
    pub jitter_ms: f32,
    pub isp: String,
    pub server: String,
    pub quality: String,
    pub quality_color: String,
}

fn calc_mbps(bytes: u64, secs: f32) -> f32 {
    if secs <= 0.0 {
        return 0.0;
    }
    (bytes as f64 * 8.0 / secs as f64 / 1_000_000.0) as f32
}

fn classify_speed(dl: f32, _ul: f32, lat: f32) -> (String, String) {
    let (label, color) = if dl > 100.0 {
        ("Excelente", "green")
    } else if dl > 50.0 {
        ("Bom", "green")
    } else if dl > 20.0 {
        ("Razoável", "yellow")
    } else if dl > 5.0 {
        ("Lento", "yellow")
    } else if lat > 200.0 {
        ("Ruim", "red")
    } else {
        ("Muito Lento", "red")
    };
    (label.to_string(), color.to_string())
}

fn emit_progress(app_handle: &Option<tauri::AppHandle>, phase: &str, pct: u8, mbps: f32) {
    if let Some(ah) = app_handle {
        let _ = ah.emit(
            "speedtest-event",
            SpeedTestProgress {
                phase: phase.to_string(),
                progress_pct: pct,
                current_mbps: mbps,
                server: SERVER_NAME.to_string(),
            },
        );
    }
}

pub fn run_speedtest(app_handle: Option<tauri::AppHandle>) -> Result<SpeedTestResult, String> {
    emit_progress(&app_handle, "latency", 0, 0.0);
    let (latency, jitter) = match measure_latency() {
        Ok(v) => v,
        Err(_) => (0.0f32, 0.0f32),
    };
    emit_progress(&app_handle, "latency", 100, 0.0);

    emit_progress(&app_handle, "download", 0, 0.0);
    let download_mbps = match test_download(app_handle.clone()) {
        Ok(mbps) => mbps,
        Err(e) => return Err(format!("Download: {}", e)),
    };
    emit_progress(&app_handle, "download", 100, download_mbps);

    emit_progress(&app_handle, "upload", 0, 0.0);
    let upload_mbps = match test_upload(app_handle.clone()) {
        Ok(mbps) => mbps,
        Err(e) => {
            eprintln!("Upload falhou (não fatal): {}", e);
            0.0
        }
    };
    emit_progress(&app_handle, "upload", 100, upload_mbps);

    let (quality, quality_color) = classify_speed(download_mbps, upload_mbps, latency);

    Ok(SpeedTestResult {
        download_mbps,
        upload_mbps,
        latency_ms: latency,
        jitter_ms: jitter,
        isp: String::new(),
        server: SERVER_NAME.to_string(),
        quality,
        quality_color,
    })
}

fn measure_latency() -> Result<(f32, f32), String> {
    let result = route::ping_host("1.1.1.1", 4)?;
    Ok((result.avg_ms, result.jitter_ms))
}

fn test_download(app_handle: Option<tauri::AppHandle>) -> Result<f32, String> {
    let total_duration = WARMUP_SECS + DOWNLOAD_SECS;
    let measure_bytes = Arc::new(AtomicU64::new(0));
    let warmup_done = Arc::new(AtomicBool::new(false));
    let start = Instant::now();
    let mut handles = Vec::new();

    for _ in 0..NUM_THREADS {
        let mb = Arc::clone(&measure_bytes);
        let wd = Arc::clone(&warmup_done);
        handles.push(std::thread::spawn(move || {
            let config = ureq::config::Config::builder()
                .timeout_global(Some(std::time::Duration::from_secs(15)))
                .build();
            let agent = ureq::Agent::new_with_config(config);

            loop {
                let elapsed = start.elapsed().as_secs_f32();
                if elapsed >= total_duration {
                    break;
                }
                let is_warmup = elapsed < WARMUP_SECS;

                let url = format!(
                    "https://speed.cloudflare.com/__down?bytes={}",
                    CHUNK_SIZE
                );
                if let Ok(resp) = agent.get(&url).call() {
                    let mut reader = resp.into_body().into_reader();
                    let mut buf = [0u8; 65536];
                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => {
                                if !is_warmup {
                                    mb.fetch_add(n as u64, Ordering::Relaxed);
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }

                if !is_warmup {
                    wd.store(true, Ordering::Relaxed);
                }
            }
        }));
    }

    if let Some(ah) = app_handle {
        let mb = Arc::clone(&measure_bytes);
        let start_clone = start;
        std::thread::spawn(move || loop {
            let elapsed = start_clone.elapsed().as_secs_f32();
            let pct = ((elapsed / total_duration) * 100.0) as u8;
            let measure_elapsed = if elapsed > WARMUP_SECS {
                elapsed - WARMUP_SECS
            } else {
                0.0
            };
            let bytes = mb.load(Ordering::Relaxed);
            let mbps = calc_mbps(bytes, measure_elapsed.max(0.5));
            let _ = ah.emit(
                "speedtest-event",
                SpeedTestProgress {
                    phase: "download".to_string(),
                    progress_pct: pct.min(99),
                    current_mbps: mbps,
                    server: SERVER_NAME.to_string(),
                },
            );
            if elapsed >= total_duration {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        });
    }

    for h in handles {
        h.join().map_err(|_| "Thread panic no download".to_string())?;
    }

    let total = measure_bytes.load(Ordering::Relaxed);
    if total == 0 {
        return Err("Nenhum dado baixado".to_string());
    }

    Ok(calc_mbps(total, DOWNLOAD_SECS))
}

fn generate_upload_data() -> Vec<u8> {
    let mut data = Vec::with_capacity(UPLOAD_CHUNK_SIZE);
    let pattern = b"KingNetworkTools Speed Test Upload Payload - ";
    let mut idx = 0usize;
    while data.len() < UPLOAD_CHUNK_SIZE {
        let remaining = UPLOAD_CHUNK_SIZE - data.len();
        let chunk = if remaining >= pattern.len() {
            pattern.len()
        } else {
            remaining
        };
        data.extend_from_slice(&pattern[..chunk]);
        data.push((idx % 256) as u8);
        data.push(b'\n');
        idx += 1;
    }
    data
}

fn test_upload(app_handle: Option<tauri::AppHandle>) -> Result<f32, String> {
    let measure_bytes = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    let mut handles = Vec::new();

    let upload_data = Arc::new(generate_upload_data());

    for _ in 0..NUM_THREADS {
        let mb = Arc::clone(&measure_bytes);
        let ud = Arc::clone(&upload_data);
        handles.push(std::thread::spawn(move || {
            let config = ureq::config::Config::builder()
                .timeout_global(Some(std::time::Duration::from_secs(15)))
                .build();
            let agent = ureq::Agent::new_with_config(config);

            loop {
                let elapsed = start.elapsed().as_secs_f32();
                if elapsed >= UPLOAD_SECS {
                    break;
                }

                match agent
                    .post("https://speed.cloudflare.com/__up")
                    .header("Content-Type", "application/octet-stream")
                    .header("Content-Length", &ud.len().to_string())
                    .send(ud.as_slice())
                {
                    Ok(resp) if {
                        let s = resp.status();
                        s == 200 || s == 204
                    } =>
                    {
                        mb.fetch_add(ud.len() as u64, Ordering::Relaxed);
                    }
                    Ok(resp) => {
                        eprintln!("Upload status: {}", resp.status());
                    }
                    Err(e) => {
                        eprintln!("Upload erro: {}", e);
                    }
                }
            }
        }));
    }

    if let Some(ah) = app_handle {
        let mb = Arc::clone(&measure_bytes);
        let start_clone = start;
        std::thread::spawn(move || loop {
            let elapsed = start_clone.elapsed().as_secs_f32();
            let pct = ((elapsed / UPLOAD_SECS) * 100.0) as u8;
            let bytes = mb.load(Ordering::Relaxed);
            let mbps = calc_mbps(bytes, elapsed.max(0.5));
            let _ = ah.emit(
                "speedtest-event",
                SpeedTestProgress {
                    phase: "upload".to_string(),
                    progress_pct: pct.min(99),
                    current_mbps: mbps,
                    server: SERVER_NAME.to_string(),
                },
            );
            if elapsed >= UPLOAD_SECS {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        });
    }

    for h in handles {
        h.join().map_err(|_| "Thread panic no upload".to_string())?;
    }

    let total = measure_bytes.load(Ordering::Relaxed);
    if total == 0 {
        return Err("Nenhum dado enviado".to_string());
    }

    Ok(calc_mbps(total, UPLOAD_SECS))
}
