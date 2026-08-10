#![allow(dead_code)]

use crate::{
    models::Storybook,
    services::pdf_images::{PdfImage, decode_png_for_pdf, image_placement, image_placement_in_box},
};
use std::{collections::HashMap, path::PathBuf};
use uuid::Uuid;

const PAGE_WIDTH: i32 = 595;
const PAGE_HEIGHT: i32 = 842;
const LEFT: i32 = 48;
const CONTENT_WIDTH: i32 = PAGE_WIDTH - LEFT * 2;

// 正文页版式：插图在上（48,440 ~ 547,770），标题与正文在下，页脚在 y=36。
const IMAGE_FRAME_X: i32 = 44;
const IMAGE_FRAME_Y: i32 = 436;
const IMAGE_FRAME_WIDTH: i32 = CONTENT_WIDTH + 8;
const IMAGE_FRAME_HEIGHT: i32 = 338;
const STORY_TEXT_TOP: i32 = 402;
const STORY_TEXT_BOTTOM: i32 = 64;

// 封面版式：边框 54,190 ~ 541,690。
const COVER_FRAME_X: i32 = 54;
const COVER_FRAME_Y: i32 = 190;
const COVER_FRAME_WIDTH: i32 = PAGE_WIDTH - COVER_FRAME_X * 2;
const COVER_FRAME_HEIGHT: i32 = 500;
const COVER_TEXT_TOP: i32 = 648;
const COVER_TEXT_BOTTOM: i32 = 220;

#[derive(Clone, Copy, PartialEq)]
enum Align {
    Left,
    Center,
}

struct PdfLine {
    text: String,
    size: i32,
    /// 与上一行基线的间距；首行相对 text_top 的下移量。
    gap: i32,
    align: Align,
}

struct PdfPage {
    background: Vec<String>,
    text_top: i32,
    text_bottom: i32,
    lines: Vec<PdfLine>,
    /// 无插图时在插图区内显示占位提示。
    image_placeholder: bool,
    footer: Option<String>,
    image: Option<usize>,
    image_box: Option<(f64, f64, f64, f64)>,
}

pub fn encode_storybook_pdf(storybook: &Storybook) -> Vec<u8> {
    encode_storybook_pdf_with_images(storybook, &HashMap::new())
}

pub fn encode_storybook_pdf_with_images(
    storybook: &Storybook,
    image_paths: &HashMap<Uuid, PathBuf>,
) -> Vec<u8> {
    let (pages, images) = storybook_pdf_pages(storybook, image_paths);
    minimal_pdf(&pages, &images, &searchable_text(storybook))
}

fn storybook_pdf_pages(
    storybook: &Storybook,
    image_paths: &HashMap<Uuid, PathBuf>,
) -> (Vec<PdfPage>, Vec<PdfImage>) {
    let mut images = Vec::new();
    let cover_image = image_paths
        .get(&storybook.id)
        .and_then(|path| decode_png_for_pdf(path).ok())
        .map(|mut image| {
            let index = images.len();
            image.name = format!("Im{}", index + 1);
            images.push(image);
            index
        });
    let mut pages = vec![cover_page(storybook, cover_image)];

    for page in &storybook.pages {
        let image = image_paths
            .get(&page.id)
            .and_then(|path| decode_png_for_pdf(path).ok())
            .map(|mut image| {
                let index = images.len();
                image.name = format!("Im{}", index + 1);
                images.push(image);
                index
            });
        pages.push(story_page(storybook, page, image));
    }

    (pages, images)
}

fn cover_page(storybook: &Storybook, image: Option<usize>) -> PdfPage {
    let role_names = storybook
        .roles
        .iter()
        .map(|role| role.name.as_str())
        .collect::<Vec<_>>()
        .join("、");

    let mut lines = vec![PdfLine {
        text: "KINDLEAF 绘本".to_string(),
        size: 12,
        gap: 18,
        align: Align::Center,
    }];
    // 书名最长两行，居中大字。
    for (index, title_line) in wrap_text(&storybook.title, 14)
        .into_iter()
        .take(2)
        .enumerate()
    {
        lines.push(PdfLine {
            text: title_line,
            size: 30,
            gap: if index == 0 { 34 } else { 36 },
            align: Align::Center,
        });
    }
    let meta = [
        format!("年龄段：{}", storybook.age_group),
        format!("使用场景：{}", storybook.use_scene),
        format!("教学目标：{}", storybook.teaching_goal),
        format!("主要角色：{}", empty_label(&role_names)),
        format!("画面风格：{}", storybook.cover_tone),
        format!("共 {} 页", storybook.pages.len()),
    ];
    for (index, item) in meta.iter().enumerate() {
        for (wrapped_index, wrapped) in wrap_text(item, 30).into_iter().enumerate() {
            lines.push(PdfLine {
                text: wrapped,
                size: 13,
                gap: if index == 0 && wrapped_index == 0 {
                    34
                } else {
                    20
                },
                align: Align::Center,
            });
        }
    }

    PdfPage {
        background: cover_background(),
        text_top: COVER_TEXT_TOP,
        text_bottom: COVER_TEXT_BOTTOM,
        lines,
        image_placeholder: false,
        footer: Some("Kindleaf 生成导出版".to_string()),
        image,
        image_box: Some((
            COVER_FRAME_X as f64 + 34.0,
            COVER_FRAME_Y as f64 + 34.0,
            COVER_FRAME_WIDTH as f64 - 68.0,
            260.0,
        )),
    }
}

fn story_page(
    storybook: &Storybook,
    page: &crate::models::StorybookPage,
    image: Option<usize>,
) -> PdfPage {
    // 交付版 PDF 只呈现读者可见内容：页码标题、正文与插图。
    // illustration_prompt 是内部生成指令，不写入成品。
    let mut lines = vec![PdfLine {
        text: format!("第 {} 页　{}", page.page_number, page.title),
        size: 18,
        gap: 0,
        align: Align::Left,
    }];
    lines.extend(wrap_text(&page.body, 33).into_iter().map(|line| PdfLine {
        text: line,
        size: 13,
        gap: 22,
        align: Align::Left,
    }));
    PdfPage {
        background: story_page_background(),
        text_top: STORY_TEXT_TOP,
        text_bottom: STORY_TEXT_BOTTOM,
        lines,
        image_placeholder: image.is_none(),
        footer: Some(format!("{} / 第 {} 页", storybook.title, page.page_number)),
        image,
        image_box: None,
    }
}

fn minimal_pdf(pages: &[PdfPage], images: &[PdfImage], searchable_text: &str) -> Vec<u8> {
    let page_count = pages.len().max(1);
    let first_page_obj = 5usize;
    let first_content_obj = first_page_obj + page_count;
    let first_image_obj = first_content_obj + page_count;
    let page_refs = (0..page_count)
        .map(|index| format!("{} 0 R", first_page_obj + index))
        .collect::<Vec<_>>()
        .join(" ");

    let mut objects = vec![
        b"1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n".to_vec(),
        format!("2 0 obj << /Type /Pages /Kids [{page_refs}] /Count {page_count} >> endobj\n")
            .into_bytes(),
        "3 0 obj << /Type /Font /Subtype /Type0 /BaseFont /STSong-Light /Encoding /UniGB-UCS2-H /DescendantFonts [4 0 R] >> endobj\n"
            .as_bytes()
            .to_vec(),
        "4 0 obj << /Type /Font /Subtype /CIDFontType0 /BaseFont /STSong-Light /CIDSystemInfo << /Registry (Adobe) /Ordering (GB1) /Supplement 2 >> >> endobj\n"
            .as_bytes()
            .to_vec(),
    ];

    for index in 0..page_count {
        let page_obj = first_page_obj + index;
        let content_obj = first_content_obj + index;
        let xobjects = pages
            .get(index)
            .and_then(|page| page.image)
            .map(|image_index| {
                let image = &images[image_index];
                let image_obj = first_image_obj + image_index;
                format!(" /XObject << /{} {} 0 R >>", image.name, image_obj)
            })
            .unwrap_or_default();
        objects.push(format!(
            "{page_obj} 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_WIDTH} {PAGE_HEIGHT}] /Resources << /Font << /F1 3 0 R >>{xobjects} >> /Contents {content_obj} 0 R >> endobj\n"
        ).into_bytes());
    }

    for (index, page) in pages.iter().enumerate() {
        let content = page_content(page, images);
        let content_obj = first_content_obj + index;
        objects.push(
            format!(
                "{content_obj} 0 obj << /Length {} >> stream\n{}endstream\nendobj\n",
                content.len(),
                content
            )
            .into_bytes(),
        );
    }

    for (index, image) in images.iter().enumerate() {
        let image_obj = first_image_obj + index;
        let header = format!(
            "{image_obj} 0 obj << /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >> stream\n",
            image.width,
            image.height,
            image.data.len()
        );
        let mut object = Vec::new();
        object.extend_from_slice(header.as_bytes());
        object.extend_from_slice(&image.data);
        object.extend_from_slice(b"\nendstream\nendobj\n");
        objects.push(object);
    }

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");
    pdf.extend_from_slice(format!("%KindleafText: {}\n", pdf_comment(searchable_text)).as_bytes());
    let mut offsets = vec![0usize];
    for object in &objects {
        offsets.push(pdf.len());
        pdf.extend_from_slice(object);
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer << /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    pdf
}

fn page_content(page: &PdfPage, images: &[PdfImage]) -> String {
    let mut content = page.background.join("");
    if let Some(image_index) = page.image {
        let image = &images[image_index];
        let placement = if let Some((x, y, width, height)) = page.image_box {
            image_placement_in_box(image, x, y, width, height)
        } else {
            image_placement(image)
        };
        content.push_str(&format!(
            "q\n{} 0 0 {} {} {} cm\n/{} Do\nQ\n",
            pdf_number(placement.width),
            pdf_number(placement.height),
            pdf_number(placement.x),
            pdf_number(placement.y),
            image.name
        ));
    } else if page.image_placeholder {
        let label = "插图待生成";
        content.push_str("BT\n/F1 14 Tf\n0.55 0.45 0.4 rg\n");
        content.push_str(&format!(
            "1 0 0 1 {} {} Tm\n<{}> Tj\n",
            pdf_number(centered_x(label, 14)),
            IMAGE_FRAME_Y + IMAGE_FRAME_HEIGHT / 2,
            utf16be_hex(label)
        ));
        content.push_str("ET\n");
    }
    content.push_str("BT\n0.2 0.14 0.12 rg\n");
    let mut y = page.text_top;
    for line in &page.lines {
        y -= line.gap;
        if y < page.text_bottom {
            break;
        }
        let x = match line.align {
            Align::Left => LEFT as f64,
            Align::Center => centered_x(&line.text, line.size),
        };
        content.push_str(&format!(
            "/F1 {} Tf\n1 0 0 1 {} {y} Tm\n",
            line.size,
            pdf_number(x)
        ));
        content.push_str(&format!("<{}> Tj\n", utf16be_hex(&line.text)));
    }
    content.push_str("ET\n");
    if let Some(footer) = &page.footer {
        content.push_str("BT\n/F1 10 Tf\n0.5 0.42 0.38 rg\n");
        content.push_str(&format!(
            "1 0 0 1 {LEFT} 36 Tm\n<{}> Tj\n",
            utf16be_hex(footer)
        ));
        content.push_str("ET\n");
    }
    content
}

/// STSong 无内嵌字宽表，用 CJK≈1em、ASCII≈0.55em 估算居中位置。
fn estimate_text_width(text: &str, size: i32) -> f64 {
    let units: f64 = text
        .chars()
        .map(|ch| if ch.is_ascii() { 0.55 } else { 1.0 })
        .sum();
    units * size as f64
}

fn centered_x(text: &str, size: i32) -> f64 {
    ((PAGE_WIDTH as f64 - estimate_text_width(text, size)) / 2.0).max(LEFT as f64)
}

fn pdf_number(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if (rounded - rounded.round()).abs() < f64::EPSILON {
        format!("{}", rounded.round() as i64)
    } else {
        format!("{rounded:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn cover_background() -> Vec<String> {
    vec![
        // 暖纸底色 + 居中边框 + 标题分隔线。
        "0.98 0.96 0.91 rg\n0 0 595 842 re f\n".to_string(),
        format!(
            "0.72 0.33 0.28 RG\n1.5 w\n{COVER_FRAME_X} {COVER_FRAME_Y} {COVER_FRAME_WIDTH} {COVER_FRAME_HEIGHT} re S\n"
        ),
        "0.72 0.33 0.28 RG\n0.8 w\n220 616 m 375 616 l S\n".to_string(),
    ]
}

fn story_page_background() -> Vec<String> {
    vec![
        "0.99 0.98 0.95 rg\n0 0 595 842 re f\n".to_string(),
        // 插图区浅色衬底（有图时会被图片盖住，无图时作为占位底）。
        format!("0.95 0.9 0.83 rg\n{LEFT} 440 {CONTENT_WIDTH} 330 re f\n"),
        format!(
            "0.72 0.33 0.28 RG\n1.2 w\n{IMAGE_FRAME_X} {IMAGE_FRAME_Y} {IMAGE_FRAME_WIDTH} {IMAGE_FRAME_HEIGHT} re S\n"
        ),
    ]
}

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch == '\n' || current.chars().count() >= max_chars {
            if !current.trim().is_empty() {
                lines.push(current.trim().to_string());
            }
            current.clear();
            if ch == '\n' {
                continue;
            }
        }
        current.push(ch);
    }
    if !current.trim().is_empty() {
        lines.push(current.trim().to_string());
    }
    if lines.is_empty() {
        lines.push(" ".to_string());
    }
    lines
}

fn utf16be_hex(text: &str) -> String {
    text.encode_utf16()
        .flat_map(|unit| [(unit >> 8) as u8, unit as u8])
        .map(|byte| format!("{byte:02X}"))
        .collect()
}

fn searchable_text(storybook: &Storybook) -> String {
    let mut text = format!(
        "{} {} {} {}",
        storybook.title, storybook.age_group, storybook.use_scene, storybook.teaching_goal
    );
    for role in &storybook.roles {
        text.push(' ');
        text.push_str(&role.name);
        text.push(' ');
        text.push_str(&role.appearance);
        text.push(' ');
        text.push_str(&role.story_function);
    }
    for page in &storybook.pages {
        text.push(' ');
        text.push_str(&page.title);
        text.push(' ');
        text.push_str(&page.body);
        text.push(' ');
        text.push_str(&page.illustration_prompt);
    }
    text
}

fn empty_label(value: &str) -> &str {
    if value.trim().is_empty() {
        "未设置"
    } else {
        value
    }
}

fn pdf_comment(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_ascii() && !matches!(ch, '\r' | '\n'))
        .take(500)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        Storybook, StorybookPage, StorybookRole, StorybookStatus, StorybookType, Visibility,
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use uuid::Uuid;

    #[test]
    fn storybook_pdf_starts_with_pdf_header_and_has_one_page_per_story_page_plus_cover() {
        let storybook = test_storybook();
        let bytes = encode_storybook_pdf(&storybook);
        let text = String::from_utf8_lossy(&bytes);

        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(text.contains("/Count 3"));
        assert!(text.contains("KindleafText: Smoke"));
    }

    #[test]
    fn utf16be_hex_preserves_chinese_text_for_type0_font() {
        assert_eq!(utf16be_hex("绘本"), "7ED8672C");
    }

    #[test]
    fn page_content_positions_each_line_below_image_area() {
        let content = page_content(
            &PdfPage {
                background: Vec::new(),
                text_top: STORY_TEXT_TOP,
                text_bottom: STORY_TEXT_BOTTOM,
                lines: vec![
                    PdfLine {
                        text: "标题".to_string(),
                        size: 18,
                        gap: 0,
                        align: Align::Left,
                    },
                    PdfLine {
                        text: "正文".to_string(),
                        size: 13,
                        gap: 22,
                        align: Align::Left,
                    },
                ],
                image_placeholder: false,
                footer: None,
                image: None,
                image_box: None,
            },
            &[],
        );

        // 标题在插图框（顶 770 / 底 436）之下，正文再往下排，不会压图。
        assert!(content.contains("1 0 0 1 48 402 Tm"));
        assert!(content.contains("1 0 0 1 48 380 Tm"));
        assert!(!content.contains(" Td"));
    }

    #[test]
    fn story_page_does_not_render_internal_illustration_prompt() {
        let storybook = test_storybook();
        let (pages, _) = storybook_pdf_pages(&storybook, &HashMap::new());
        let story_page = &pages[1];
        let content = page_content(story_page, &[]);

        assert!(!content.contains(&utf16be_hex("温暖幼儿园教室，纸感水彩。")));
        assert!(content.contains(&utf16be_hex("孩子们一起讨论怎样轮流玩。")));
        assert!(content.contains(&utf16be_hex("第 1 页　小汽车来到教室")));
    }

    #[test]
    fn story_page_without_image_draws_placeholder_label() {
        let storybook = test_storybook();
        let (pages, _) = storybook_pdf_pages(&storybook, &HashMap::new());
        let content = page_content(&pages[1], &[]);

        assert!(content.contains(&utf16be_hex("插图待生成")));
    }

    #[test]
    fn text_that_overflows_page_bottom_is_clipped() {
        let lines = (0..30)
            .map(|_| PdfLine {
                text: "一行正文".to_string(),
                size: 13,
                gap: 22,
                align: Align::Left,
            })
            .collect::<Vec<_>>();
        let content = page_content(
            &PdfPage {
                background: Vec::new(),
                text_top: STORY_TEXT_TOP,
                text_bottom: STORY_TEXT_BOTTOM,
                lines,
                image_placeholder: false,
                footer: None,
                image: None,
                image_box: None,
            },
            &[],
        );

        // 从 y=402 开始每行下移 22，最后一行不低于 64：402-22*n >= 64 → n <= 15。
        assert!(!content.contains("1 0 0 1 48 60 Tm"));
        assert!(!content.contains("1 0 0 1 48 46 Tm"));
    }

    #[test]
    fn storybook_pdf_contains_picture_book_layout_marks() {
        let storybook = test_storybook();
        let bytes = encode_storybook_pdf(&storybook);
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("44 436 507 338 re S"));
        assert!(text.contains("48 440 499 330 re f"));
        assert!(text.contains("KindleafText: Smoke"));
    }

    #[test]
    fn storybook_pdf_embeds_latest_page_image_when_available() {
        let storybook = test_storybook();
        let page_id = storybook.pages[0].id;
        let image_path = std::env::temp_dir().join(format!("kindleaf-pdf-{page_id}.png"));
        write_test_transparent_png(&image_path);
        let decoded = decode_png_for_pdf(&image_path);
        assert!(
            decoded.is_ok(),
            "test PNG should be supported by PDF image decoder: {:?}",
            decoded.err()
        );
        let mut images = std::collections::HashMap::new();
        images.insert(page_id, image_path.clone());

        let bytes = encode_storybook_pdf_with_images(&storybook, &images);
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("/Subtype /Image"));
        assert!(text.contains("/Filter /DCTDecode"));
        assert!(text.contains("/XObject << /Im1"));
        assert!(text.contains("/Im1 Do"));
        let _ = std::fs::remove_file(image_path);
    }

    #[test]
    fn pdf_image_placement_preserves_square_image_ratio() {
        let image = PdfImage {
            name: "Im1".to_string(),
            width: 1920,
            height: 1920,
            data: Vec::new(),
        };

        let placement = image_placement(&image);

        assert_eq!(pdf_number(placement.width), "330");
        assert_eq!(pdf_number(placement.height), "330");
        assert_eq!(pdf_number(placement.x), "132.5");
        assert_eq!(pdf_number(placement.y), "440");
    }

    #[test]
    fn pdf_image_placement_preserves_wide_and_tall_ratios() {
        let wide = PdfImage {
            name: "Im1".to_string(),
            width: 1600,
            height: 800,
            data: Vec::new(),
        };
        let tall = PdfImage {
            name: "Im2".to_string(),
            width: 800,
            height: 1600,
            data: Vec::new(),
        };

        let wide_placement = image_placement(&wide);
        let tall_placement = image_placement(&tall);

        assert_eq!(pdf_number(wide_placement.width), "499");
        assert_eq!(pdf_number(wide_placement.height), "249.5");
        assert_eq!(pdf_number(wide_placement.x), "48");
        assert_eq!(pdf_number(tall_placement.width), "165");
        assert_eq!(pdf_number(tall_placement.height), "330");
        assert_eq!(pdf_number(tall_placement.x), "215");
    }

    #[test]
    fn page_content_uses_aspect_fit_image_transform() {
        let content = page_content(
            &PdfPage {
                background: Vec::new(),
                text_top: STORY_TEXT_TOP,
                text_bottom: STORY_TEXT_BOTTOM,
                lines: Vec::new(),
                image_placeholder: false,
                footer: None,
                image: Some(0),
                image_box: None,
            },
            &[PdfImage {
                name: "Im1".to_string(),
                width: 1920,
                height: 1920,
                data: Vec::new(),
            }],
        );

        assert!(content.contains("330 0 0 330 132.5 440 cm"));
        assert!(content.contains("/Im1 Do"));
        assert!(!content.contains("499 0 0 330 48 440 cm"));
    }

    #[test]
    fn cover_title_is_centered() {
        let storybook = test_storybook();
        let (pages, _) = storybook_pdf_pages(&storybook, &HashMap::new());
        let content = page_content(&pages[0], &[]);

        // “Smoke 测试绘本” 宽 ≈ (6*0.55 + 4) * 30 = 219 → x ≈ (595-219)/2 = 188
        assert!(content.contains("/F1 30 Tf\n1 0 0 1 188"));
        assert!(content.contains(&utf16be_hex("KINDLEAF 绘本")));
    }

    fn write_test_transparent_png(path: &std::path::Path) {
        std::fs::write(
            path,
            BASE64_STANDARD
                .decode("iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGP4cGnfsxNbGCAUAEWMCcWN1afmAAAAAElFTkSuQmCC")
                .unwrap(),
        )
        .unwrap();
    }

    fn test_storybook() -> Storybook {
        Storybook {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            title: "Smoke 测试绘本".to_string(),
            storybook_type: StorybookType::Plain,
            status: StorybookStatus::Exportable,
            visibility: Visibility::Private,
            source: "blank".to_string(),
            source_title: None,
            target_child_id: None,
            creator_name: "林老师".to_string(),
            updated_at: "刚刚".to_string(),
            age_group: "4-5 岁".to_string(),
            use_scene: "课堂共读".to_string(),
            teaching_goal: "学习轮流".to_string(),
            cover_tone: "温暖纸感".to_string(),
            teacher_review_status: "pending".to_string(),
            teacher_reviewed_by: None,
            teacher_reviewed_at: None,
            pages: vec![
                StorybookPage {
                    id: Uuid::new_v4(),
                    page_number: 1,
                    title: "小汽车来到教室".to_string(),
                    body: "孩子们一起讨论怎样轮流玩。".to_string(),
                    illustration_prompt: "温暖幼儿园教室，纸感水彩。".to_string(),
                    status: "ready".to_string(),
                    image_url: None,
                    selected_image_variant_id: None,
                },
                StorybookPage {
                    id: Uuid::new_v4(),
                    page_number: 2,
                    title: "朋友也想玩".to_string(),
                    body: "老师引导大家说出自己的想法。".to_string(),
                    illustration_prompt: "老师和孩子围坐。".to_string(),
                    status: "ready".to_string(),
                    image_url: None,
                    selected_image_variant_id: None,
                },
            ],
            roles: vec![StorybookRole {
                id: Uuid::new_v4(),
                name: "林老师".to_string(),
                role_type: "teacher".to_string(),
                appearance: "温柔老师".to_string(),
                story_function: "引导规则".to_string(),
                needs_consistency: true,
                reference_image_url: None,
                reference_image_prompt: None,
                reference_status: "not_started".to_string(),
                selected_image_variant_id: None,
            }],
            quality: Default::default(),
        }
    }
}
