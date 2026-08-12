pub const DEFAULT_PAGE_ASPECT_RATIO: &str = "portrait_4_5";
const MIN_SEEDREAM_PIXELS: u64 = 3_686_400;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageAspectSpec {
    pub key: &'static str,
    pub image_size: &'static str,
    pub pdf_width: i32,
    pub pdf_height: i32,
    pub prompt_clause: &'static str,
}

pub fn normalize_page_aspect_ratio(value: Option<&str>) -> String {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some("landscape_16_9") => "landscape_16_9".to_string(),
        Some("square_1_1") => "square_1_1".to_string(),
        Some("portrait_4_5") | _ => DEFAULT_PAGE_ASPECT_RATIO.to_string(),
    }
}

pub fn page_aspect_spec(value: &str) -> PageAspectSpec {
    match normalize_page_aspect_ratio(Some(value)).as_str() {
        "landscape_16_9" => PageAspectSpec {
            key: "landscape_16_9",
            image_size: "2560x1440",
            pdf_width: 842,
            pdf_height: 474,
            prompt_clause: "页面比例：横版 16:9，适合课堂大屏展示；按本页镜头景别安排主体大小与环境占比，重要内容保留安全边距，避免贴边。",
        },
        "square_1_1" => PageAspectSpec {
            key: "square_1_1",
            image_size: "1920x1920",
            pdf_width: 720,
            pdf_height: 720,
            prompt_clause: "页面比例：方形 1:1，适合卡片式绘本；按本页镜头景别安排主体大小与环境占比，四周保留安全边距，画面不要过度拥挤。",
        },
        _ => PageAspectSpec {
            key: "portrait_4_5",
            image_size: "1792x2240",
            pdf_width: 595,
            pdf_height: 842,
            prompt_clause: "页面比例：竖版 4:5，适合单页绘本阅读；按本页镜头景别安排主体大小与环境占比，重要内容保留安全边距，避免贴边。",
        },
    }
}

pub fn valid_seedream_size(value: &str) -> bool {
    let Some((width, height)) = value.trim().split_once('x') else {
        return false;
    };
    let Ok(width) = width.parse::<u64>() else {
        return false;
    };
    let Ok(height) = height.parse::<u64>() else {
        return false;
    };
    width.saturating_mul(height) >= MIN_SEEDREAM_PIXELS
}

pub fn image_size_for_aspect_with_fallback(
    aspect_ratio: Option<&str>,
    requested_size: Option<&str>,
) -> Option<String> {
    if let Some(size) = requested_size
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if valid_seedream_size(size) {
            return Some(size.to_string());
        }
    }
    aspect_ratio
        .map(page_aspect_spec)
        .map(|aspect| aspect.image_size.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_aspect_image_sizes_meet_seedream_minimum() {
        for key in ["portrait_4_5", "landscape_16_9", "square_1_1"] {
            assert!(valid_seedream_size(page_aspect_spec(key).image_size));
        }
    }

    #[test]
    fn image_size_fallback_replaces_legacy_small_sizes() {
        assert_eq!(
            image_size_for_aspect_with_fallback(Some("landscape_16_9"), Some("1344x768")),
            Some("2560x1440".to_string())
        );
        assert_eq!(
            image_size_for_aspect_with_fallback(Some("portrait_4_5"), Some("1024x1280")),
            Some("1792x2240".to_string())
        );
    }
}
