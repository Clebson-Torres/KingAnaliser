use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct QualityThresholds {
    pub excellent_max: f32,
    pub good_max: f32,
    pub acceptable_max: f32,
    pub loss_none: f32,
    pub loss_low_max: f32,
}

pub fn classify_latency(avg_ms: f32) -> &'static str {
    match avg_ms as u32 {
        0..=4 => "Excelente",
        5..=29 => "Bom",
        30..=79 => "Aceitável",
        _ => "Ruim",
    }
}

#[allow(dead_code)]
pub fn classify_loss(pct: f32) -> &'static str {
    if pct == 0.0 {
        "Sem perda"
    } else if pct <= 2.0 {
        "Perda baixa"
    } else {
        "Perda alta"
    }
}

#[allow(dead_code)]
pub fn quality_color(quality: &str) -> &'static str {
    match quality {
        "Excelente" | "Bom" | "ok" => "green",
        "Aceitável" | "warning" => "yellow",
        _ => "red",
    }
}

pub fn get_thresholds() -> QualityThresholds {
    QualityThresholds {
        excellent_max: 4.0,
        good_max: 29.0,
        acceptable_max: 79.0,
        loss_none: 0.0,
        loss_low_max: 2.0,
    }
}
