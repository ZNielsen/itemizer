// © Zach Nielsen 2024

use rusqlite::{params, Connection, Result};
use regex::Regex;

use std::collections::HashMap;
use std::path::Path;
use std::cmp::max;
use std::ops::{Deref, DerefMut};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;

pub trait Itemizer {
    fn process_purchase(&mut self, code: u64, desc: String, price: f64);
}
pub struct DatabaseItemizer {
    pub db: Connection,
}
pub struct FileItemizer {
    pub maps: ItemMaps,
    pub purchases: Purchases,
}

#[derive(Clone, Debug)]
pub struct ItemRule {
    pub code: u64,
    pub desc: String,
    pub name: String,
    pub tags: Vec<String>,
    pub excl: bool,
}
pub struct ItemMaps {
    pub codes: HashMap<u64, usize>,
    pub descr: HashMap<String, usize>,
    pub rules: Vec<ItemRule>,
}
pub struct Purchase {
    pub name: String,
    pub tags: Vec<String>,
    pub price: f64,
}
pub struct Purchases(Vec<Purchase>);

pub enum ReceiptType {
    FredMeyer,
    Costco
}
pub struct Receipt {
    pub text: String,
    pub re: Regex,
}

///////////////////////////////////////////////////////////////////////////////////////////////////

pub fn get_env(var: &str) -> String {
    env::var(var).expect(&format!("Env var not found: [{}]", var))
}

pub fn image_done(image: &str) -> bool {
    let done_file = get_env("ITEMIZER_IMAGE_DONE_FILE");
    let done = std::fs::read_to_string(done_file).unwrap();
    for line in done.lines() {
        if line == image {
            return true;
        }
    }
    false
}

pub fn split_tags(tags: &str) -> Vec<String> {
    tags
        .split(",")
        .map(|s| s.trim().to_string())
        .collect()
}

///////////////////////////////////////////////////////////////////////////////////////////////////

impl ItemRule {
    pub fn new() -> Self {
        Self {
            code: 0,
            desc: String::new(),
            name: String::new(),
            tags: Vec::new(),
            excl: false,
        }
    }
}


impl Receipt {
    pub fn new(text: String) -> Self {
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

    pub fn get_fields(&self, line: &str) -> Option<(u64, String, f64)> {
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

impl DatabaseItemizer {
    pub fn new() -> Self {
        let db_path = get_env("ITEMIZER_DB");
        let s = Self {
            db: Connection::open(db_path).unwrap(),
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

    pub fn create_table(&self, table: &str) {
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
    fn process_purchase(&mut self, code: u64, desc: String, price: f64) {
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

impl ItemMaps {
    pub fn init() -> Self {
        let mut codes = HashMap::new();
        let mut descr = HashMap::new();
        let mut rules = Vec::new();

        let rules_file = get_env("ITEMIZER_RULES_FILE");
        let text = std::fs::read_to_string(rules_file).unwrap();
        for group in text.split("\n\n") {
            if group.starts_with("//") {
                continue;
            }
            let mut item = ItemRule::new();
            let sg: Vec<&str> = group.lines().collect();
            // Format:
            //   code
            //   desc
            //   name
            //   tags // optional
            //   excl // optional
            item.code = sg[0].parse().unwrap();
            item.desc = sg[1].to_owned();
            item.name = sg[2].to_owned();

            if sg.len() == 5 {
                item.tags = split_tags(sg[3]);
                item.excl = sg[4].parse().unwrap();
            } else if sg.len() == 4 {
                let excl: Result<bool, std::str::ParseBoolError> = sg[3].parse();
                if excl.is_ok() {
                    item.excl = excl.unwrap();
                } else {
                    item.tags = split_tags(sg[3]);
                }
            }

            if codes.contains_key(&item.code) || descr.contains_key(&item.desc) {
                println!("WARNING: duplicate item found in rules list: [{:?}]", item);
            }
            codes.insert(item.code, rules.len());
            descr.insert(item.desc.clone(), rules.len());
            rules.push(item);
        }

        Self {codes, descr, rules}
    }
}

impl Purchases {
    pub fn init() -> Self {
        let mut v = Vec::new();
        let purchases_file = get_env("ITEMIZER_PURCHASES_FILE");
        let text = std::fs::read_to_string(purchases_file).unwrap();
        for line in text.lines() {
            let parts: Vec<&str> = line
                .split("|")
                .map(|s| s.trim())
                .collect();

            let price = parts[0].parse().unwrap();
            let name = parts[1].to_owned();
            let tags = split_tags(parts[2]);

            v.push(Purchase{price, name, tags});
        }

        Purchases(v)
    }
}
impl Deref for Purchases {
    type Target = Vec<Purchase>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Purchases {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FileItemizer {
    pub fn new() -> Self {
        Self {
            maps: ItemMaps::init(),
            purchases: Purchases::init(),
        }
    }

    pub fn save_to_files(&self) {
        // Purchases File
        let purchases_path = get_env("ITEMIZER_PURCHASES_FILE");
        let (price_max, name_max, tags_max) = self.get_max_lengths();
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(purchases_path)
            .unwrap();
        for p in &self.purchases.0 {
            // Price | Name | Tags
            let s = format!("{:<price_max$} | {:<name_max$} | {:<tags_max$}",
                p.price, p.name, p.tags.join(", "));
            file.write_all(s.as_bytes()).unwrap();
        }

        // Rules File
        let rules_path = get_env("ITEMIZER_RULES_FILE");
    }

    pub fn get_max_lengths(&self) -> (usize, usize, usize) {
        // Get length of each field
        let mut price_max = 0;
        let mut name_max = 0;
        let mut tags_max = 0;
        for p in &self.purchases.0 {
            price_max = max(price_max, p.price.to_string().len());
            name_max = max(name_max, p.name.len());
            let mut this_tags_len = max(0, p.tags.len()-1 * 2); // Account for `, ` between each entry
            for tag in &p.tags {
                this_tags_len += tag.len();
            }
            tags_max = max(tags_max, this_tags_len);
        }

        (price_max, name_max, tags_max)
    }
}
impl Itemizer for FileItemizer {
    fn process_purchase(&mut self, code: u64, desc: String, price: f64) {
        // Get item index
        let idx = if self.maps.codes.contains_key(&code) {
            self.maps.codes[&code]
        } else if self.maps.descr.contains_key(&desc) {
            self.maps.descr[&desc]
        } else {
            // TODO: Ask for manual input, append to file
            println!("No item for code/desc/price: [{}]/[{}]/[{}]", code, desc, price);
            println!("Inserting entry into rules file");
            self.maps.codes.insert(code, self.maps.rules.len());
            self.maps.descr.insert(desc.clone(), self.maps.rules.len());
            self.maps.rules.push(ItemRule{code, desc, name: "UNKNOWN".to_owned(), tags: Vec::new(), excl: true});
            self.maps.rules.len()-1
        };

        // Insert item into list of purchases
        self.purchases.push(Purchase {
            name: self.maps.rules[idx].name.clone(),
            tags: self.maps.rules[idx].tags.clone(),
            price,
        });
    }
}

