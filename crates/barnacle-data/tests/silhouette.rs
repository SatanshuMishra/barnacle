use barnacle_data::extract::silhouette::BACKGROUND;
use barnacle_data::extract::silhouette::composite;
use barnacle_data::extract::silhouette::sha256_hex;
use image::ImageFormat;
use image::Rgba;
use image::RgbaImage;

fn encode(image: &RgbaImage) -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    image.write_to(&mut cursor, ImageFormat::Png).unwrap();
    cursor.into_inner()
}

#[test]
fn transparent_pixels_take_the_background_and_opaque_pixels_stay() {
    let white = Rgba([255, 255, 255, 255]);
    let clear = Rgba([0, 0, 0, 0]);
    let source = RgbaImage::from_fn(404, 155, |x, _| if x < 200 { white } else { clear });
    let result = image::load_from_memory(&composite(&encode(&source), BACKGROUND).unwrap())
        .unwrap()
        .to_rgba8();
    assert_eq!(result.dimensions(), (404, 155));
    assert_eq!(*result.get_pixel(10, 10), white);
    assert_eq!(*result.get_pixel(300, 10), BACKGROUND);
}

#[test]
fn bytes_that_are_not_a_png_are_rejected() {
    assert!(composite(b"not a png", BACKGROUND).is_err());
}

#[test]
fn hash_is_lowercase_sha256_hex() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}
