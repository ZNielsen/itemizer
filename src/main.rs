use tesseract::Tesseract;
use rusqlite::{params, Connection, Result};
use image::imageops::FilterType;
use image::GenericImageView;
use regex::Regex;

use std::collections::HashMap;
use std::path::Path;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;

trait Itemizer {
    fn process_purchase(&self, code: u64, desc: String, price: f64);
}

struct DatabaseItemizer {
    db: Connection,
    // rules: ItemDicts,
}

#[derive(Clone, Debug)]
struct ItemRule {
    code: u64,
    desc: String,
    name: String,
    excl: bool,
}
impl ItemRule {
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
    codes: HashMap<u64, ItemRule>,
    descr: HashMap<String, ItemRule>
}

enum ReceiptType {
    FredMeyer,
    Costco
}
struct Receipt {
    text: String,
    re: Regex,
}
impl Receipt {
    fn new(text: String) -> Self {
        let fm_list = ["fredmeyer", "fred meyer"];
        let co_list = ["costco", "wholesale"];
        let lower_text = text.to_lowercase();
        let store = if fm_list.iter().any(|&s| lower_text.contains(s)) {
            ReceiptType::FredMeyer
        } else if co_list.iter().any(|&s| lower_text.contains(s)) {
            ReceiptType::Costco
        } else {
            panic!("Could not recognize receipt type: {}", text);
        };

        let pattern = match store {
            ReceiptType::Costco => r"(\d+) ([\w -]+) (\d?\d\.\d\d)",
            ReceiptType::FredMeyer => r"(\d+) ([\w ]+) (\d?\d\.\d\d) F",
        };
        let re = Regex::new(pattern).unwrap();

        Self { text, re }
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
    let mut itemizer = DatabaseItemizer::new();
    let image_dir = env::var("ITEMIZER_IMAGE_DIR").expect("Env var not found: ITEMIZER_IMAGE_DIR");

    for entry in std::fs::read_dir(image_dir).unwrap() {
        let entry = entry.unwrap();
        let entry_path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            // TODO: Recurse into directory?
            continue;
        }
        let entry_str = entry_path.as_os_str().to_str().unwrap();
        if image_done(entry_str) {
            println!("image already done, skipping: {}", entry_str);
            continue;
        }

        println!("About to open: {:?}", &entry_path);
        let resized_path =
            [env::var("ITEMIZER_UPSCALED_IMAGE_DIR").expect("Env var not found: ITEMIZER_UPSCALED_IMAGE_DIR"),
            entry.file_name().into_string().unwrap()]
                .join("/");
        if std::fs::metadata(&resized_path).is_ok() {
            println!("Already upscaled [{}], moving on", resized_path);
        } else {
            let img = image::open(&entry_path).unwrap();
            let (width, height) = img.dimensions();
            println!("About to upscale: {:?}", &entry_path);
            let upscale = 1.5;
            let new_width = (width as f32 * upscale) as u32;
            let new_height = (height as f32 * upscale) as u32;
            // Resize the image using the Lanczos3 filter
            let resized_img = image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);
            println!("About to save upscaled to: {}", &resized_path);
            resized_img.save(&resized_path).unwrap();
            println!("Image has been upscaled and saved successfully.");
        }

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

            println!("Checking for [{}], [{}]", code, desc);
            itemizer.process_purchase(code, desc, price)
        }

        let done_file = env::var("ITEMIZER_IMAGE_DONE_FILE").expect("Env var not found: ITEMIZER_IMAGE_DONE_FILE");
        let mut done_fp = OpenOptions::new()
            .append(true)
            .open(done_file).unwrap();
        writeln!(done_fp, "{}", entry_str).unwrap();
    }
}

impl DatabaseItemizer {
    fn new() -> Self {
        let db_path = env::var("ITEMIZER_DB").expect("Env var not found: ITEMIZER_DB");
        let s = Self {
            db: Connection::open(db_path).unwrap(),
            // rules: load_rules(),
        };


        let tables = ["items", "tags", "items_tags", "purchases"];
        for table in tables {
            let table_exists: bool = s.db.query_row(
                &format!("SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type='table' AND name='{}')", table),
                [],
                |row| row.get(0),
            ).unwrap();
            if !table_exists {
                println!("The '{}' table does not exist, creating all tables", table);
                s.create_table(table);
            }
        }

        // Update all items from rules file
        // {
        //     let mut stmt = s.db.prepare("INSERT INTO items (code, desc, name, excl) VALUES (?, ?, ?, ?)").unwrap();
        //     for (_, rule) in s.rules.codes.clone() {
        //         stmt.execute(params![rule.code, rule.desc, rule.name, rule.excl]).unwrap();
        //     }
        // }

        s
    }

    fn create_table(&self, table: &str) {
        match table {
            "items" => {
                self.db.execute(
                    "CREATE TABLE items (
                            item_id INTEGER PRIMARY KEY,
                            code INTEGER UNIQUE,
                            desc TEXT NOT NULL,
                            name TEXT NOT NULL,
                            excl INTEGER DEFAULT 0
                        )",
                    [],
                ).unwrap();
            }
            "tags" => {
                self.db.execute(
                    "CREATE TABLE tags (
                            tag_id   INTEGER PRIMARY KEY AUTOINCREMENT,
                            tag_name TEXT NOT NULL UNIQUE
                        )",
                    [],
                ).unwrap();
            }
            "items_tags" => {
                self.db.execute(
                    "CREATE TABLE items_tags (
                            item_id  INTEGER,
                            tag_id   INTEGER,
                            FOREIGN KEY (item_id) REFERENCES items(item_id),
                            FOREIGN KEY (tag_id)  REFERENCES tags(tag_id),
                            PRIMARY KEY (item_id, tag_id)
                        )",
                    [],
                ).unwrap();
            }
            "purchases" => {
                self.db.execute(
                    "CREATE TABLE purchases (
                            purchase_id   INTEGER PRIMARY KEY AUTOINCREMENT,
                            item_id       INTEGER,
                            price         REAL,
                            purchase_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                            FOREIGN KEY (item_id) REFERENCES items(item_id)
                        )",
                    [],
                ).unwrap();
            }
            _ => panic!(),
        }
    }
}
impl Itemizer for DatabaseItemizer {
    fn process_purchase(&self, code: u64, desc: String, price: f64) {
        let mut stmt = self.db.prepare("SELECT item_id FROM items WHERE code = ?1").unwrap();
        let item_id = match stmt.query_row(params![code], |row| row.get(0)) {
            Ok(id) => id,
            Err(e) => {
                println!("could not find code: [{}]", e);
                let mut stmt = self.db.prepare("SELECT item_id FROM items WHERE desc = ?1").unwrap();
                match stmt.query_row(params![desc], |row| row.get(0)) {
                    Ok(id) => id,
                    Err(_) => {
                        // TODO: Ask for manual input, append to file
                        println!("No item for code/desc/price: [{}]/[{}]/[{}]", code, desc, price);
                        // panic!("No item for code/desc [{}]/[{}]: {}", code, desc, e);
                        // Insert into items table so I just have to backfill name
                        self.db.execute(
                            "INSERT INTO items (code, desc, name) VALUES (?1, ?2, ?3)",
                            (
                                &code,
                                &desc,
                                ""
                            ),
                        ).unwrap();
                        self.db.last_insert_rowid()
                    }
                }
            }
        };
        self.db.execute(
            "INSERT INTO purchases (item_id, price) VALUES (?1, ?2)",
            (
                &item_id,
                &price,
            ),
        ).unwrap();
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

fn load_rules() -> ItemDicts {
    let mut codes = HashMap::new();
    let mut descr = HashMap::new();

    let dict_file = env::var("ITEMIZER_RULES_FILE").expect("Env var not found: ITEMIZER_RULES_FILE");
    let text = std::fs::read_to_string(dict_file).unwrap();
    for group in text.split("\n\n") {
        if group.contains("code:\ndesc:\nname:") || group.starts_with("//") {
            continue;
        }
        let mut item = ItemRule::new();
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

fn name_is_in_table(conn: &Connection, table: &str, name: &str) -> bool {
    let mut stmt = conn.prepare(&format!("SELECT COUNT(*) FROM {} WHERE name = ?", table)).unwrap();
    let count: i64 = stmt.query_row(params![name], |row| row.get(0)).unwrap();
    count > 0
}

