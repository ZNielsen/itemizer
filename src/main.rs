use tesseract::Tesseract;
use image::imageops::FilterType;
use image::GenericImageView;

use std::collections::HashMap;
use std::path::Path;
use std::env;

struct Dict {
    codes: HashMap<u32, String>,
    descr: HashMap<String, String>
}

enum ReceiptType {
    FredMeyer,
    Costco
}
impl ReceiptType {
    fn get_fields(&self) -> (u32, String, f64) {
    }
}

fn main() {
    let dict = load_dict();
    let _ = scan_files_in_dir(&dict, Path::new(&env::var("ITEMIZER_IMAGE_DIR").unwrap()));
}

fn scan_files_in_dir(dict: &Dict, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            scan_files_in_dir(&dict, &entry.path())?;
            continue;
        }
        if image_done(&entry) {
            continue;
        }

        println!("About to upscale: {:?}", entry.path());
        let img = image::open(entry.path()).unwrap();
        let (width, height) = img.dimensions();
        let upscale = 1.5;
        let new_width = (width as f32 * upscale) as u32;
        let new_height = (height as f32 * upscale) as u32;
        // Resize the image using the Lanczos3 filter
        let resized_img = image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);
        let resized_path =
            [env::var("ITEMIZER_UPSCALED_IMAGE_FOLDER")?,
            entry.file_name().into_string().unwrap()]
                .join("/");
        resized_img.save(&resized_path).unwrap();
        println!("Image has been upscaled and saved successfully.");

        let tess = Tesseract::new(None, Some("eng")).unwrap();
        let mut tess = tess.set_image(&resized_path).unwrap();
        // let mut tess = tess.set_image(entry.path().as_os_str().to_str().unwrap()).unwrap();
        let text = tess.get_text().unwrap();
        println!("Recognized text:\n{}", text);
        // TODO: Delete upscaled image


        // Get type of receipt
        // Grab the itemized part. bounded by receipt type?

        for line in text.lines() {
            let (code, desc, price) = receipt.get_fields();
            if dict.codes.contains_key(code) {
                // TODO: Where am I keeping this data?
            } else if dict.descr.contains_key(desc) {
                // TODO: Where am I keeping this data?
            } else {
                panic!("Could not find entry: {}", line);
                // TODO: Ask for manual input, append to file
            }
        }
    }
    Ok(())
}

fn image_done(image: &std::fs::DirEntry) -> bool {
    let done_file = env::var("ITEMIZER_IMAGE_DONE_FILE").unwrap();
    let done = std::fs::read_to_string(done_file).unwrap();
    for line in done.lines() {
        if line == image.path().as_os_str() {
            return true;
        }
    }
    false
}

fn load_dict() -> Dict {
    let mut codes = HashMap::new();
    let mut descr = HashMap::new();

    let dict_file = env::var("ITEMIZER_DICT_FILE").unwrap();
    let text = std::fs::read_to_string(dict_file).unwrap();
    for group in text.split("\n\n") {
        let items: Vec<&str> = group.lines().collect();
        codes.insert(items[0].parse().unwrap(), items[2].to_owned());
        descr.insert(items[1].to_owned(), items[2].to_owned());
    }

    Dict {codes, descr}
}
