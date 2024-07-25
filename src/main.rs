use tesseract::Tesseract;
use rusqlite::{Connection, Result};
use image::imageops::FilterType;
use image::GenericImageView;
use regex::Regex;

use std::collections::HashMap;
use std::path::Path;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;

struct Itemizer {
    db: Connection,
    items: ItemDicts,
}

#[derive(Clone, Debug)]
struct Item {
    code: u64,
    desc: String,
    name: String,
    excl: bool,
}
impl Item {
    fn new() -> Self {
        Self {
            code: 0,
            desc: String::new(),
            name: String::new(),
            excl: false
        }
    }
}
struct ItemDicts {
    codes: HashMap<u64, Item>,
    descr: HashMap<String, Item>
}

enum ReceiptType {
    FredMeyer,
    Costco
}
struct Receipt {
    store: ReceiptType,
    text: String,
    re: Regex,
}
impl Receipt {
    fn new(text: String) -> Self {
        let store = if text.to_lowercase().contains("fredmeyer") {
            ReceiptType::FredMeyer
        } else if text.to_lowercase().contains("wholesale") {
            ReceiptType::Costco
        } else {
            panic!("Could not recognize receipt type: {}", text);
        };

        let pattern = match store {
            ReceiptType::Costco => r"(\d+) ([\w -]+) (\d?\d\.\d\d)",
            ReceiptType::FredMeyer => r"(\d+) ([\w ]+) (\d?\d\.\d\d) F",
        };
        let re = Regex::new(pattern).unwrap();

        Self { store, text, re }
    }
    fn get_fields(&self, line: &str) -> Option<(u64, String, f64)> {
        if let Some(caps) = self.re.captures(line) {
            println!(
                "got fields from line: [{}], [{}], [{}]",
                caps[1].to_owned(), caps[2].to_owned(), caps[3].to_owned()
            );
            Some((
                caps[1].parse().unwrap(),
                caps[2].to_owned(),
                caps[3].parse().unwrap()
            ))
        } else {
            println!("No regex match on line: [{}]", line);
            None
        }
    }
}

fn main() {
    let mut itemizer = Itemizer::new();
    let image_dir = env::var("ITEMIZER_IMAGE_DIR").expect("Env var not found: ITEMIZER_IMAGE_DIR");
    let res = itemizer.scan_files_in_dir(Path::new(&image_dir));
    // TODO: Save database to file
    if let Err(e) = res {
        panic!("{:?}", e);
    }
}

impl Itemizer{
fn new() -> Self {
    let s = Self {
        db: Connection::open_in_memory().unwrap(),
        items: load_items(),
    };


    // Check if the 'person' table exists.
    let table_exists: bool = s.db.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type='table' AND name='items')",
        [],
        |row| row.get(0),
    ).unwrap();

    if table_exists {
        println!("Loaded in memory DB");
    } else {
        println!("The 'items' table does not exist, creating it in-memory");
        // TODO: tags? broader categories beyond the name?
        s.db.execute(
            "CREATE TABLE items (
                    id       INTEGER PRIMARY KEY AUTOINCREMENT,
                    code     INTEGER,
                    name     TEXT NOT NULL,
                    price    REAL NOT NULL
                )",
            [],
        ).unwrap();
    }

    s
}
fn scan_files_in_dir(&mut self, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let entry_path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            self.scan_files_in_dir(&entry_path)?;
            continue;
        }
        let entry_str = entry_path.as_os_str().to_str().unwrap();
        if image_done(entry_str) {
            println!("image already done, skipping: {}", entry_str);
            continue;
        }

        println!("About to open: {:?}", &entry_path);
        let img = image::open(&entry_path).unwrap();
        let (width, height) = img.dimensions();
        println!("About to upscale: {:?}", &entry_path);
        let upscale = 1.5;
        let new_width = (width as f32 * upscale) as u32;
        let new_height = (height as f32 * upscale) as u32;
        // Resize the image using the Lanczos3 filter
        let resized_img = image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);
        let resized_path =
            [env::var("ITEMIZER_UPSCALED_IMAGE_DIR").expect("Env var not found: ITEMIZER_UPSCALED_IMAGE_DIR"),
            entry.file_name().into_string().unwrap()]
                .join("/");
        println!("About to save upscaled to: {}", &resized_path);
        resized_img.save(&resized_path).unwrap();
        println!("Image has been upscaled and saved successfully.");

        let tess = Tesseract::new(None, Some("eng")).unwrap();
        let mut tess = tess.set_image(&resized_path).unwrap();
        // let mut tess = tess.set_image(entry_str).unwrap();
        let text = tess.get_text().unwrap();
        println!("Recognized text:\n{}", text);
        // TODO: Delete upscaled image


        let receipt = Receipt::new(text);
        for line in receipt.text.lines() {
            let Some((code, desc, price)) = receipt.get_fields(line) else {
                continue;
            };
            if self.items.codes.contains_key(&code) {
                self.db.execute(
                    "INSERT INTO items (code, name, price) VALUES (?1, ?2, ?3)",
                    (
                        &code,
                        &self.items.codes[&code].name,
                        &price,
                    ),
                )?;
            } else if self.items.descr.contains_key(&desc) {
                self.db.execute(
                    "INSERT INTO items (code, name, price) VALUES (?1, ?2, ?3)",
                    (
                        &self.items.descr[&desc].code,
                        &self.items.descr[&desc].name,
                        &price,
                    ),
                )?;
            } else {
                // panic!("Could not find entry: {}", line);
                println!("Could not find entry: {}", line);
                // TODO: Ask for manual input, append to file
            }
        }

        let done_file = env::var("ITEMIZER_IMAGE_DONE_FILE").expect("Env var not found: ITEMIZER_IMAGE_DONE_FILE");
        let mut done_fp = OpenOptions::new()
            .append(true)
            .open(done_file)?;
        writeln!(done_fp, "{}", entry_str)?;
    }
    Ok(())
}
}

fn image_done(image: &str) -> bool {
    let done_file = env::var("ITEMIZER_IMAGE_DONE_FILE").expect("Env var not found: ITEMIZER_IMAGE_DONE_FILE");
    let done = std::fs::read_to_string(done_file).unwrap();
    for line in done.lines() {
        if line == image {
            return true;
        }
    }
    false
}

fn load_items() -> ItemDicts {
    let mut codes = HashMap::new();
    let mut descr = HashMap::new();

    let dict_file = env::var("ITEMIZER_ITEMS_FILE").expect("Env var not found: ITEMIZER_ITEMS_FILE");
    let text = std::fs::read_to_string(dict_file).unwrap();
    for group in text.split("\n\n") {
        if group.contains("code:\ndesc:\nname:") || group.starts_with("//") {
            continue;
        }
        let mut item = Item::new();
        let settings: Vec<&str> = group.lines().collect();
        for setting in settings {
            let split = setting.find(":").unwrap();
            let (key, val) = setting.split_at(split);
            match key {
                "code" => item.code = val.trim_start_matches(":").parse().unwrap(),
                "desc" => item.desc = val.trim_start_matches(":").to_owned(),
                "name" => item.name = val.trim_start_matches(":").to_owned(),
                "excl" => item.excl = val.trim_start_matches(":").parse().unwrap(),
                _ => panic!("Unexpected setting key: {}", key),
            }
        }
        if codes.contains_key(&item.code) || descr.contains_key(&item.desc) {
            println!("WARNING: duplicate item found in rules list: [{:?}]", item);
        }
        codes.insert(item.code, item.clone());
        descr.insert(item.desc.clone(), item);
    }

    ItemDicts {codes, descr}
}

