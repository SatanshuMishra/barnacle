use image::ImageFormat;
use image::Rgba;
use image::RgbaImage;
use sha2::Digest;
use sha2::Sha256;

pub const BACKGROUND: Rgba<u8> = Rgba([0xD3, 0xE6, 0xE1, 0xFF]);

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn composite(
    silhouette_png: &[u8],
    background: Rgba<u8>,
) -> Result<Vec<u8>, image::ImageError> {
    let silhouette =
        image::load_from_memory_with_format(silhouette_png, ImageFormat::Png)?.to_rgba8();
    let mut canvas = RgbaImage::from_pixel(silhouette.width(), silhouette.height(), background);
    image::imageops::overlay(&mut canvas, &silhouette, 0, 0);
    let mut encoded = std::io::Cursor::new(Vec::new());
    canvas.write_to(&mut encoded, ImageFormat::Png)?;
    Ok(encoded.into_inner())
}
