use std::{fs::File, path::Path};

const IMAGE_BOX_X: f64 = 64.0;
const IMAGE_BOX_Y: f64 = 468.0;
const IMAGE_BOX_WIDTH: f64 = 467.0;
const IMAGE_BOX_HEIGHT: f64 = 226.0;

pub(crate) struct PdfImage {
    pub(crate) name: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgb: Vec<u8>,
}

pub(crate) struct ImagePlacement {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

pub(crate) fn image_placement(image: &PdfImage) -> ImagePlacement {
    let image_width = image.width.max(1) as f64;
    let image_height = image.height.max(1) as f64;
    let image_ratio = image_width / image_height;
    let box_ratio = IMAGE_BOX_WIDTH / IMAGE_BOX_HEIGHT;
    let (width, height) = if image_ratio >= box_ratio {
        (IMAGE_BOX_WIDTH, IMAGE_BOX_WIDTH / image_ratio)
    } else {
        (IMAGE_BOX_HEIGHT * image_ratio, IMAGE_BOX_HEIGHT)
    };

    ImagePlacement {
        x: IMAGE_BOX_X + (IMAGE_BOX_WIDTH - width) / 2.0,
        y: IMAGE_BOX_Y + (IMAGE_BOX_HEIGHT - height) / 2.0,
        width,
        height,
    }
}

pub(crate) fn decode_png_for_pdf(path: &Path) -> Result<PdfImage, String> {
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
