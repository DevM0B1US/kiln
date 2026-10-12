use std::path::PathBuf;

use ember_core::models::RMBG_14;
use ember_core::{MatteOptions, MergeFit, ModelStore, Segmenter};
use image::{DynamicImage, GenericImageView, RgbaImage};

fn models_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../models")
}

fn work_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ember-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir is writable");
    dir
}

fn test_photo() -> DynamicImage {
    let mut pixels = RgbaImage::new(128, 128);
    for (x, y, pixel) in pixels.enumerate_pixels_mut() {
        let inside = (32..96).contains(&x) && (32..96).contains(&y);
        *pixel = if inside {
            image::Rgba([200, 40, 40, 255])
        } else {
            image::Rgba([240, 240, 240, 255])
        };
    }
    DynamicImage::ImageRgba8(pixels)
}

fn test_template() -> RgbaImage {
    let mut template = RgbaImage::from_pixel(64, 64, image::Rgba([20, 20, 20, 255]));
    for y in 16..48 {
        for x in 16..48 {
            template.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
        }
    }
    template
}

#[test]
fn remove_pipeline_runs_on_the_real_model() {
    let store = ModelStore::new(models_root());
    assert!(
        store.is_installed(RMBG_14.id),
        "dev models symlink is present"
    );
    let handle = store
        .get(RMBG_14.id)
        .expect("pinned hash verifies and session loads");

    let dir = work_dir();
    let input = dir.join("photo.png");
    let photo = test_photo();
    photo.save(&input).expect("temp dir is writable");

    let source = ember_core::image_io::read(&input).expect("just wrote it");
    let matte = Segmenter::new(&handle)
        .matte_for(&source, RMBG_14.input_size)
        .expect("inference runs");
    assert_eq!(matte.dimensions(), source.dimensions());

    let options = MatteOptions {
        threshold: None,
        feather: 0.0,
        dilate: 0,
        trim: false,
    };
    let refined = ember_core::matte::refine(&matte, options);
    let rgba = ember_core::image_io::apply_matte(&source, &refined).expect("matte matches image");
    let output = dir.join("cutout.png");
    ember_core::image_io::write_png(&DynamicImage::ImageRgba8(rgba), &output)
        .expect("confined fresh path writes");
    assert!(output.is_file());

    let leftovers = std::fs::read_dir(&dir)
        .expect("temp dir is readable")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn merge_pipeline_composites_at_template_size() {
    let dir = work_dir();
    let photo_path = dir.join("photo.png");
    let template_path = dir.join("template.png");
    test_photo()
        .save(&photo_path)
        .expect("temp dir is writable");
    DynamicImage::ImageRgba8(test_template())
        .save(&template_path)
        .expect("temp dir is writable");

    let photo = ember_core::image_io::read(&photo_path).expect("just wrote it");
    let template = ember_core::image_io::read(&template_path).expect("just wrote it");
    let merged = ember_core::composite(&photo.to_rgba8(), &template.to_rgba8(), MergeFit::Cover)
        .expect("non-empty inputs composite");
    let (width, height) = template.dimensions();
    assert_eq!((merged.width(), merged.height()), (width, height));

    let output = dir.join("merged.png");
    ember_core::image_io::write_png(&DynamicImage::ImageRgba8(merged), &output)
        .expect("confined fresh path writes");
    assert!(output.is_file());
}
