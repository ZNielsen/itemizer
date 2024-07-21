use tesseract::Tesseract;
use image::GenericImageView;
use image::imageops::FilterType;

use std::path::Path;

const IMAGE_FOLDER: &str = "/Users/z/iCloud/itemizer_images";
const UPSCALED_IMAGE_FOLDER: &str = "/Users/z/code/itemizer/upscaled_images";
const IMAGE_DONE_FILE: &str = "/Users/z/code/itemizer/done_images";

fn main() {
    let _ = scan_files_in_dir(Path::new(IMAGE_FOLDER));
}

fn scan_files_in_dir(dir: &Path) -> Result<(), ()> {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            scan_files_in_dir(&entry.path())?;
            continue;
        }
        if image_done(&entry) {
            continue;
        }

        println!("About to upscale: {:?}", entry.path());
        let img = image::open(entry.path()).unwrap();
        let (width, height) = img.dimensions();
        let new_width = (width as f32 * 2.0) as u32;
        let new_height = (height as f32 * 2.0) as u32;
        // Resize the image using the Lanczos3 filter
        let resized_img = image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);
        let resized_path =
            [UPSCALED_IMAGE_FOLDER,
            &entry.file_name().into_string().unwrap()]
                .join("/").to_owned();
        resized_img.save(&resized_path).unwrap();
        println!("Image has been upscaled and saved successfully.");

        let tess = Tesseract::new(None, Some("eng")).unwrap();
        let mut tess = tess.set_image(&resized_path).unwrap();
        // let mut tess = tess.set_image(entry.path().as_os_str().to_str().unwrap()).unwrap();
        let text = tess.get_text().unwrap();
        println!("Recognized text:\n{}", text);

        // for line in text.lines() {
        // }
    }
    Ok(())
}

fn image_done(image: &std::fs::DirEntry) -> bool {
    for line in std::fs::read_to_string(IMAGE_DONE_FILE).unwrap().lines() {
        if line == image.path().as_os_str() {
            return true;
        }
    }
    false
}
