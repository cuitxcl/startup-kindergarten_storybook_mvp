#![allow(dead_code)]

use crate::models::Storybook;
use std::{collections::HashMap, fs::File, path::PathBuf};
use uuid::Uuid;

const PAGE_WIDTH: i32 = 595;
const PAGE_HEIGHT: i32 = 842;
const LEFT: i32 = 48;
const TOP: i32 = 790;
const LINE_HEIGHT: i32 = 24;
const CONTENT_WIDTH: i32 = PAGE_WIDTH - LEFT * 2;

struct PdfPage {
    background: Vec<String>,
    lines: Vec<(String, i32)>,
    footer: Option<String>,
    image: Option<usize>,
}

struct PdfImage {
    name: String,
    width: u32,
    height: u32,
    rgb: Vec<u8>,
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
    let role_names = storybook
        .roles
        .iter()
        .map(|role| role.name.as_str())
        .collect::<Vec<_>>()
        .join("、");
    let mut pages = vec![PdfPage {
        background: cover_background(),
        lines: vec![
            ("Kindleaf 绘本导出".to_string(), 22),
            (storybook.title.clone(), 28),
            (format!("年龄段：{}", storybook.age_group), 14),
            (format!("使用场景：{}", storybook.use_scene), 14),
            (format!("教学目标：{}", storybook.teaching_goal), 14),
            (format!("主要角色：{}", empty_label(&role_names)), 14),
            (format!("画面风格：{}", storybook.cover_tone), 14),
            (format!("共 {} 页", storybook.pages.len()), 14),
        ],
        footer: Some("Kindleaf 生成导出版".to_string()),
        image: None,
    }];
    let mut images = Vec::new();

    for page in &storybook.pages {
        let mut lines = vec![
            (format!("第 {} 页", page.page_number), 14),
            (page.title.clone(), 24),
        ];
        lines.extend(wrap_text(&page.body, 28).into_iter().map(|line| (line, 15)));
        lines.push(("插图画面".to_string(), 14));
        lines.extend(
            wrap_text(&page.illustration_prompt, 30)
                .into_iter()
                .map(|line| (line, 12)),
        );
        let image = image_paths
            .get(&page.id)
            .and_then(|path| decode_png_for_pdf(path).ok())
            .map(|mut image| {
                let index = images.len();
                image.name = format!("Im{}", index + 1);
                images.push(image);
                index
            });
        pages.push(PdfPage {
            background: story_page_background(),
            lines,
            footer: Some(format!("{} / 第 {} 页", storybook.title, page.page_number)),
            image,
        });
    }

    (pages, images)
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
            "{image_obj} 0 obj << /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {} >> stream\n",
            image.width,
            image.height,
            image.rgb.len()
        );
        let mut object = Vec::new();
        object.extend_from_slice(header.as_bytes());
        object.extend_from_slice(&image.rgb);
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
        content.push_str(&format!(
            "q\n467 0 0 226 64 468 cm\n/{} Do\nQ\n",
            image.name
        ));
    }
    content.push_str("BT\n");
    let mut y = TOP;
    for (index, (line, size)) in page.lines.iter().enumerate() {
        if index > 0 {
            y -= LINE_HEIGHT;
        }
        content.push_str(&format!("/F1 {size} Tf\n1 0 0 1 {LEFT} {y} Tm\n"));
        content.push_str(&format!("<{}> Tj\n", utf16be_hex(line)));
    }
    content.push_str("ET\n");
    if let Some(footer) = &page.footer {
        content.push_str("BT\n/F1 10 Tf\n");
        content.push_str(&format!(
            "1 0 0 1 {LEFT} 36 Tm\n<{}> Tj\n",
            utf16be_hex(footer)
        ));
        content.push_str("ET\n");
    }
    content
}

fn decode_png_for_pdf(path: &PathBuf) -> Result<PdfImage, String> {
    let file = File::open(path).map_err(|err| format!("打开 PNG 失败：{err}"))?;
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|err| format!("读取 PNG 信息失败：{err}"))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|err| format!("解码 PNG 失败：{err}"))?;
    let bytes = &buf[..info.buffer_size()];
    let rgb = png_bytes_to_rgb(bytes, info.color_type, info.bit_depth)?;
    Ok(PdfImage {
        name: String::new(),
        width: info.width,
        height: info.height,
        rgb,
    })
}

fn png_bytes_to_rgb(
    bytes: &[u8],
    color_type: png::ColorType,
    bit_depth: png::BitDepth,
) -> Result<Vec<u8>, String> {
    if bit_depth != png::BitDepth::Eight {
        return Err("PDF 导出暂只支持 8-bit PNG 插图".to_string());
    }
    match color_type {
        png::ColorType::Rgb => Ok(bytes.to_vec()),
        png::ColorType::Rgba => Ok(bytes
            .chunks_exact(4)
            .flat_map(|chunk| {
                let alpha = u16::from(chunk[3]);
                [0, 1, 2].map(move |index| {
                    let foreground = u16::from(chunk[index]);
                    let blended = (foreground * alpha + 255 * (255 - alpha)) / 255;
                    blended as u8
                })
            })
            .collect()),
        png::ColorType::Grayscale => Ok(bytes.iter().flat_map(|value| [*value; 3]).collect()),
        png::ColorType::GrayscaleAlpha => Ok(bytes
            .chunks_exact(2)
            .flat_map(|chunk| {
                let alpha = u16::from(chunk[1]);
                let foreground = u16::from(chunk[0]);
                let blended = ((foreground * alpha + 255 * (255 - alpha)) / 255) as u8;
                [blended; 3]
            })
            .collect()),
        png::ColorType::Indexed => Err("PDF 导出暂不支持调色板 PNG 插图".to_string()),
    }
}

fn cover_background() -> Vec<String> {
    vec![
        // Warm paper background and title panel.
        "0.98 0.96 0.91 rg\n0 0 595 842 re f\n".to_string(),
        format!("0.72 0.33 0.28 RG\n1.5 w\n{LEFT} 438 {CONTENT_WIDTH} 284 re S\n"),
        "0.92 0.78 0.67 rg\n64 454 467 84 re f\n".to_string(),
    ]
}

fn story_page_background() -> Vec<String> {
    vec![
        "0.99 0.98 0.95 rg\n0 0 595 842 re f\n".to_string(),
        format!("0.72 0.33 0.28 RG\n1 w\n{LEFT} 452 {CONTENT_WIDTH} 258 re S\n"),
        "0.94 0.86 0.78 rg\n64 468 467 226 re f\n".to_string(),
        "0.86 0.67 0.56 RG\n0.8 w\n64 468 467 226 re S\n".to_string(),
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
    fn page_content_positions_each_line_absolutely() {
        let content = page_content(
            &PdfPage {
                background: Vec::new(),
                lines: vec![("标题".to_string(), 22), ("正文".to_string(), 14)],
                footer: None,
                image: None,
            },
            &[],
        );

        assert!(content.contains("1 0 0 1 48 790 Tm"));
        assert!(content.contains("1 0 0 1 48 766 Tm"));
        assert!(!content.contains(" Td"));
    }

    #[test]
    fn storybook_pdf_contains_picture_book_layout_marks() {
        let storybook = test_storybook();
        let bytes = encode_storybook_pdf(&storybook);
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("48 452 499 258 re S"));
        assert!(text.contains("64 468 467 226 re f"));
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
        assert!(text.contains("/XObject << /Im1"));
        assert!(text.contains("/Im1 Do"));
        let _ = std::fs::remove_file(image_path);
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
            pages: vec![
                StorybookPage {
                    id: Uuid::new_v4(),
                    page_number: 1,
                    title: "小汽车来到教室".to_string(),
                    body: "孩子们一起讨论怎样轮流玩。".to_string(),
                    illustration_prompt: "温暖幼儿园教室，纸感水彩。".to_string(),
                    status: "ready".to_string(),
                },
                StorybookPage {
                    id: Uuid::new_v4(),
                    page_number: 2,
                    title: "朋友也想玩".to_string(),
                    body: "老师引导大家说出自己的想法。".to_string(),
                    illustration_prompt: "老师和孩子围坐。".to_string(),
                    status: "ready".to_string(),
                },
            ],
            roles: vec![StorybookRole {
                id: Uuid::new_v4(),
                name: "林老师".to_string(),
                role_type: "teacher".to_string(),
                appearance: "温柔老师".to_string(),
                story_function: "引导规则".to_string(),
                needs_consistency: true,
            }],
        }
    }
}
