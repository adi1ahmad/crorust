use crorust_common::{
    FilterType, RasterResizer, ResampleConfigurationBuilder, TransformConstraint,
};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use std::io::Cursor;

fn create_test_master_icon() -> tempfile::NamedTempFile {
    let file = tempfile::Builder::new()
        .suffix(".png")
        .tempfile()
        .expect("failed to create temp master icon");

    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(512, 512);
    for (x, _y, pixel) in img.enumerate_pixels_mut() {
        if x < 256 {
            *pixel = Rgba([255, 255, 255, 255]);
        } else {
            *pixel = Rgba([0, 0, 0, 255]);
        }
    }

    img.save(file.path())
        .expect("failed to save temp master icon");
    file
}

fn create_transparent_master_icon() -> tempfile::NamedTempFile {
    let file = tempfile::Builder::new()
        .suffix(".png")
        .tempfile()
        .expect("failed to create temp master icon");

    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(512, 512);
    let center = 256.0f32;
    let radius = 200.0f32;

    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let dx = x as f32 - center;
        let dy = y as f32 - center;
        if (dx * dx + dy * dy).sqrt() <= radius {
            *pixel = Rgba([255, 0, 0, 255]);
        } else {
            *pixel = Rgba([0, 0, 0, 0]);
        }
    }

    img.save(file.path())
        .expect("failed to save transparent icon");
    file
}

#[test]
fn test_scaler_downscaling_antialiasing_vs_nearest() {
    let master = create_test_master_icon();
    let scaler = RasterResizer::open(master.path()).expect("failed to open scaler");

    let lanczos_bytes = scaler.to_vec(TransformConstraint::new(32));
    let lanczos_img =
        image::load_from_memory(&lanczos_bytes).expect("failed to decode lanczos png");

    let nearest_bytes =
        scaler.to_vec(TransformConstraint::new(32).with_filter(FilterType::Nearest));
    let nearest_img =
        image::load_from_memory(&nearest_bytes).expect("failed to decode nearest png");

    assert_eq!(lanczos_img.dimensions(), (32, 32));
    assert_eq!(nearest_img.dimensions(), (32, 32));

    let mut nearest_has_intermediate = false;
    for x in 0..32 {
        let luma = nearest_img.get_pixel(x, 16)[0];
        if luma > 0 && luma < 255 {
            nearest_has_intermediate = true;
        }
    }
    assert!(
        !nearest_has_intermediate,
        "nearest neighbor should only have extreme values"
    );

    let mut lanczos_has_intermediate = false;
    for x in 0..32 {
        let luma = lanczos_img.get_pixel(x, 16)[0];
        if luma > 0 && luma < 255 {
            lanczos_has_intermediate = true;
            break;
        }
    }
    assert!(
        lanczos_has_intermediate,
        "lanczos3 must produce intermediate antialiased pixel values"
    );
}

#[test]
fn test_scaler_alpha_preservation_and_padding() {
    let master = create_transparent_master_icon();
    let scaler = RasterResizer::open(master.path()).expect("failed to open scaler");

    let opts = ResampleConfigurationBuilder::new(48, 48)
        .padding(0.1)
        .build();

    let bytes = scaler.to_vec(opts);
    let img = image::load_from_memory(&bytes).expect("failed to load resized image");
    assert_eq!(img.dimensions(), (48, 48));

    assert_eq!(img.get_pixel(0, 0)[3], 0);
    assert_eq!(img.get_pixel(47, 0)[3], 0);
    assert_eq!(img.get_pixel(0, 47)[3], 0);
    assert_eq!(img.get_pixel(47, 47)[3], 0);

    let center_pixel = img.get_pixel(24, 24);
    assert_eq!(center_pixel[0], 255);
    assert_eq!(center_pixel[3], 255);
}

#[test]
fn test_scaler_color_optimization_and_dimensions() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;

    let gray_path = dir.path().join("gray.png");
    let gray_img = ImageBuffer::from_pixel(512, 512, Rgba([128u8, 128, 128, 255]));
    DynamicImage::ImageRgba8(gray_img).save(&gray_path)?;

    let mut scaler = RasterResizer::open(&gray_path)?;
    scaler.optimize();
    let out_bytes = scaler.try_to_vec(
        ResampleConfigurationBuilder::new(64, 64)
            .filter(FilterType::CatmullRom)
            .build(),
    )?;
    let decoded = image::load_from_memory(&out_bytes)?;
    assert_eq!(decoded.dimensions(), (64, 64));
    assert_eq!(decoded.color(), image::ColorType::L8);

    let rgb_path = dir.path().join("rgb.png");
    let master = create_test_master_icon();
    std::fs::copy(master.path(), &rgb_path)?;
    let mut rgb_scaler = RasterResizer::open(&rgb_path)?;
    rgb_scaler.optimize();
    let rect_bytes = rgb_scaler.try_to_vec(ResampleConfigurationBuilder::new(120, 60).build())?;
    let rect_decoded = image::load_from_memory(&rect_bytes)?;
    assert_eq!(rect_decoded.dimensions(), (120, 60));

    Ok(())
}

#[test]
fn test_scaler_disk_cache_serves_cached_bytes_without_rerendering() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let master = create_test_master_icon();
    let cache_dir = dir.path().join("icon_cache");

    let scaler = RasterResizer::open(master.path())?.with_cache_dir(&cache_dir);
    let opts = TransformConstraint::new(48);

    let mut first_buf = Vec::new();
    scaler.write(&mut Cursor::new(&mut first_buf), opts)?;

    let expected_cached_file = cache_dir
        .join(scaler.source_hash())
        .join("w48_h48_s48_lanczos3_raw.png");
    assert!(expected_cached_file.is_file());
    assert_eq!(std::fs::read(&expected_cached_file)?, first_buf);

    let sentinel = b"CACHED_PNG_SENTINEL_BYTES";
    std::fs::write(&expected_cached_file, sentinel)?;

    let scaler2 = RasterResizer::open(master.path())?.with_cache_dir(&cache_dir);
    let mut second_buf = Vec::new();
    scaler2.write(&mut Cursor::new(&mut second_buf), opts)?;
    assert_eq!(second_buf, sentinel);

    Ok(())
}
