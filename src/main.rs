// © Zach Nielsen 2024

mod data;
use crate::data::*;

use tesseract::Tesseract;
use image::imageops::FilterType;
use image::GenericImageView;

fn main() {
    let image_dir = get_env("ITEMIZER_IMAGE_DIR");
    let done_file = get_env("ITEMIZER_IMAGE_DONE_FILE");

    let mut itemizer = FileItemizer::new();
    // let mut itemizer = DatabaseItemizer::new();
    for entry in std::fs::read_dir(image_dir).unwrap() {
        let entry = entry.unwrap();
        let entry_path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            // TODO: Recurse into directory?
            continue;
        }
        let entry_path = entry_path.as_os_str().to_str().unwrap();
        if image_done(entry_path) {
            println!("Receipt already done, skipping: {}", entry_path);
            continue;
        }

        // Upscale image
        let entry_name = entry.file_name().into_string().unwrap();
        let resized_path = resize_image(entry_path, entry_name);

        // OCR image
        let tess = Tesseract::new(None, Some("eng")).unwrap();
        let mut tess = tess.set_image(&resized_path).unwrap();
        // let mut tess = tess.set_image(entry_str).unwrap();
        let text = tess.get_text().unwrap();
        println!("Recognized text:\n{}", text);
        // TODO: Delete upscaled image

        // Parse Receipt
        let receipt = Receipt::new(text);
        for line in receipt.text.lines() {
            let Some((code, desc, price)) = receipt.get_fields(line) else {
                continue;
            };

            println!("Checking for [{}], [{}]", code, desc);
            itemizer.process_purchase(code, desc, price)
        }

        // let mut done_fp = OpenOptions::new()
        //     .append(true)
        //     .open(&done_file).unwrap();
        // writeln!(done_fp, "{}", entry_path).unwrap();
    }

    itemizer.print_totals();
    itemizer.save_to_disk();
}

fn resize_image(path: &str, name: String) -> String {
    println!("About to open: {:?}", path);
    let resized_path =
        [get_env("ITEMIZER_UPSCALED_IMAGE_DIR"), name].join("/");
    if std::fs::metadata(&resized_path).is_ok() {
        println!("Already upscaled [{}], moving on", resized_path);
    } else {
        let img = image::open(&path).unwrap();
        let (width, height) = img.dimensions();
        println!("About to upscale: {:?}", &path);
        let upscale = 1.5;
        let new_width = (width as f32 * upscale) as u32;
        let new_height = (height as f32 * upscale) as u32;
        // Resize the image using the Lanczos3 filter
        let resized_img = image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);
        println!("About to save upscaled to: {}", &resized_path);
        resized_img.save(&resized_path).unwrap();
        println!("Image has been upscaled and saved successfully.");
    }
    resized_path
}

