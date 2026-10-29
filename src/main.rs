//! Kiln entry point: the window, or a headless pipeline harness.
//!
//! `kiln`            — the desktop app
//! `kiln cut`        — remove backgrounds, headless
//! `kiln merge`      — template merge, headless

mod app;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use kiln::{MatteOptions, MergeFit, ModelStore, image_io, matte as matte_math};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("cut") => headless_cut(&args[1..]),
        Some("merge") => headless_merge(&args[1..]),
        Some("--help" | "-h" | "help") => {
            print_help();
            ExitCode::SUCCESS
        }
        _ => run_window(),
    }
}

fn print_help() {
    println!(
        "Kiln — fast desktop background remover plus template merger.\n\
         \n\
         Usage:\n\
         \x20 kiln                       open the window\n\
         \x20 kiln cut <images...> --out <dir> [--threshold N] [--feather F] [--dilate N] [--trim]\n\
         \x20 kiln merge <template.png> --out <dir> <photos...> [--fit cover|fit|stretch]\n\
         \n\
         The RMBG-1.4 weights (personal use only, BRIA licence) go in models/ \n\
         next to the binary, or set KILN_MODELS_DIR."
    );
}

fn models_root() -> ModelStore {
    ModelStore::new(ModelStore::default_root())
}

fn headless_cut(args: &[String]) -> ExitCode {
    let mut images = Vec::new();
    let mut out = None;
    let mut options = MatteOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out = args.get(i).map(PathBuf::from);
            }
            "--threshold" => {
                i += 1;
                options.threshold = args
                    .get(i)
                    .and_then(|v| v.parse::<u32>().ok())
                    .and_then(|v| u8::try_from(v).ok());
            }
            "--feather" => {
                i += 1;
                options.feather = args
                    .get(i)
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0);
            }
            "--dilate" => {
                i += 1;
                options.dilate = args.get(i).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
            }
            "--trim" => options.trim = true,
            other => images.push(PathBuf::from(other)),
        }
        i += 1;
    }
    let Some(output_dir) = out else {
        eprintln!("kiln cut: --out <dir> is required");
        return ExitCode::from(2);
    };
    let store = models_root();
    let mut failures = 0;
    for path in &images {
        let result = (|| -> Result<String, String> {
            let source = image_io::read(path).map_err(|error| error.to_string())?;
            let handle = store
                .get(kiln::models::RMBG_14.id)
                .map_err(|error| error.to_string())?;
            let matte = kiln::Segmenter::new(&handle)
                .matte_for(&source, kiln::models::RMBG_14.input_size)
                .map_err(|error| error.to_string())?;
            let refined = matte_math::refine(&matte, options);
            let mut rgba =
                image_io::apply_matte(&source, &refined).map_err(|error| error.to_string())?;
            if options.trim
                && let Some(cropped) = matte_math::trim(&rgba, &refined)
            {
                rgba = cropped;
            }
            write_numbered(path, &output_dir, &image::DynamicImage::ImageRgba8(rgba))
        })();
        match result {
            Ok(path) => println!("ok   {path}"),
            Err(message) => {
                failures += 1;
                eprintln!("fail {}: {message}", path.display());
            }
        }
    }
    exit_code(failures)
}

fn headless_merge(args: &[String]) -> ExitCode {
    let mut template = None;
    let mut photos = Vec::new();
    let mut out = None;
    let mut fit = MergeFit::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out = args.get(i).map(PathBuf::from);
            }
            "--fit" => {
                i += 1;
                fit = args
                    .get(i)
                    .map_or(MergeFit::default(), |label| match label.as_str() {
                        "cover" => MergeFit::Cover,
                        "fit" => MergeFit::Fit,
                        "stretch" => MergeFit::Stretch,
                        _ => MergeFit::default(),
                    });
            }
            other => {
                if template.is_none() {
                    template = Some(PathBuf::from(other));
                } else {
                    photos.push(PathBuf::from(other));
                }
            }
        }
        i += 1;
    }
    let (Some(template_path), Some(output_dir)) = (template, out) else {
        eprintln!("kiln merge: <template.png> and --out <dir> are required");
        return ExitCode::from(2);
    };
    let mut failures = 0;
    let template_image = image_io::read(&template_path).map_err(|error| error.to_string());
    for photo in &photos {
        let result = match &template_image {
            Err(message) => Err(message.clone()),
            Ok(template_pixels) => (|| -> Result<String, String> {
                let photo_image = image_io::read(photo).map_err(|error| error.to_string())?;
                let merged =
                    kiln::composite(&photo_image.to_rgba8(), &template_pixels.to_rgba8(), fit)
                        .map_err(|error| error.to_string())?;
                write_numbered(photo, &output_dir, &image::DynamicImage::ImageRgba8(merged))
            })(),
        };
        match result {
            Ok(path) => println!("ok   {path}"),
            Err(message) => {
                failures += 1;
                eprintln!("fail {}: {message}", photo.display());
            }
        }
    }
    exit_code(failures)
}

fn write_numbered(
    input: &Path,
    output_dir: &Path,
    image: &image::DynamicImage,
) -> Result<String, String> {
    let numbered = kiln::registry::output_path(input, output_dir, "png");
    let name = numbered.file_name().map_or_else(
        || "output.png".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let dest =
        kiln::registry::confined_path(output_dir, &name).map_err(|error| error.to_string())?;
    image_io::write_png(image, &dest).map_err(|error| error.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}

fn exit_code(failures: usize) -> ExitCode {
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn run_window() -> ExitCode {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Kiln")
            .with_decorations(false)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 600.0]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "Kiln",
        options,
        Box::new(|creation_context| {
            app::fonts::install(&creation_context.egui_ctx);
            Ok(Box::new(app::KilnApp::new(models_root())))
        }),
    )
    .map_or(ExitCode::FAILURE, |()| ExitCode::SUCCESS)
}
