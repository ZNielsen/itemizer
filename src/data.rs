// © Zach Nielsen 2024

use crate::config::Config;

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use regex::Regex;

use std::collections::HashMap;
use std::cmp::max;
use std::ops::{Deref, DerefMut};
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub struct FileItemizer {
    pub config: Config,
    pub maps: ItemMaps,
    pub purchases: Purchases,
    pub current_date: NaiveDate,
}

#[derive(Clone, Debug)]
pub struct ItemRule {
    pub code: u64,
    pub desc: String,
    pub name: String,
    pub tags: Vec<String>,
}
pub struct ItemMaps {
    pub codes: HashMap<u64, usize>,
    pub descr: HashMap<String, usize>,
    pub rules: Vec<ItemRule>,
}
#[derive(Clone, Debug)]
pub struct Purchase {
    pub name: String,
    pub tags: Vec<String>,
    pub price: f64,
    pub date: NaiveDate,
    pub code: Option<u64>,
}
pub struct Purchases(pub Vec<Purchase>);

pub enum ReceiptType {
    FredMeyer,
    Costco,
    WinCo,
}
pub struct Receipt {
    pub store: ReceiptType,
    pub text: String,
    pub re: Regex,
}

///////////////////////////////////////////////////////////////////////////////////////////////////

pub fn image_done(image: &str, done_file: &Path) -> Result<bool> {
    match std::fs::read_to_string(done_file) {
        Ok(content) => Ok(content.lines().any(|line| line == image)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e).with_context(|| format!("Failed to read done file: {}", done_file.display())),
    }
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
        }
    }
}

impl Receipt {
    pub fn new(text: String) -> Result<Self> {
        let fm_list = ["fredmeyer", "fred meyer"];
        let co_list = ["costco", "wholesale"];
        let wc_list = ["winco"];
        let lower_text = text.to_lowercase();
        let store = if fm_list.iter().any(|&s| lower_text.contains(s)) {
            ReceiptType::FredMeyer
        } else if co_list.iter().any(|&s| lower_text.contains(s)) {
            ReceiptType::Costco
        } else if wc_list.iter().any(|&s| lower_text.contains(s)) {
            ReceiptType::WinCo
        } else {
            let preview: String = text.lines().take(5).collect::<Vec<_>>().join("\n");
            bail!("Could not identify store from receipt text:\n{}", preview);
        };

        let pattern = match store {
            ReceiptType::Costco => r"(\d+) ([\w ./'&()-]+) (\d{1,4}[.,]\d\d)",
            ReceiptType::FredMeyer => r"(\d+) ([\w <+./'&()-]+) (\d{1,4}[.,]\d\d) [A-Z]",
            ReceiptType::WinCo => r"([\w .,/'&()-]+) (\d+) (\d{1,4}[.,]\d\d)",
        };
        let re = Regex::new(pattern).unwrap();

        Ok(Self { store, text, re })
    }

    pub fn get_fields(&self, line: &str) -> Option<(u64, String, f64)> {
        let caps = self.re.captures(line)?;
        match self.store {
            ReceiptType::Costco |
            ReceiptType::FredMeyer => {
                let code: u64 = match caps[1].parse() {
                    Ok(c) => c,
                    Err(_) => {
                        eprintln!("  Skipping line, bad code '{}': [{}]", &caps[1], line);
                        return None;
                    }
                };
                let price: f64 = match caps[3].replace(",", ".").parse() {
                    Ok(p) => p,
                    Err(_) => {
                        eprintln!("  Skipping line, bad price '{}': [{}]", &caps[3], line);
                        return None;
                    }
                };
                Some((code, caps[2].to_owned(), price))
            },
            ReceiptType::WinCo => {
                let code: u64 = match caps[2].parse() {
                    Ok(c) => c,
                    Err(_) => {
                        eprintln!("  Skipping line, bad code '{}': [{}]", &caps[2], line);
                        return None;
                    }
                };
                let price: f64 = match caps[3].replace(",", ".").parse() {
                    Ok(p) => p,
                    Err(_) => {
                        eprintln!("  Skipping line, bad price '{}': [{}]", &caps[3], line);
                        return None;
                    }
                };
                Some((code, caps[1].to_owned(), price))
            },
        }
    }
}

impl ItemMaps {
    pub fn init(rules_path: &Path) -> Result<Self> {
        let mut codes = HashMap::new();
        let mut descr = HashMap::new();
        let mut rules = Vec::new();

        let text = match std::fs::read_to_string(rules_path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self { codes, descr, rules });
            }
            Err(e) => {
                return Err(e).with_context(|| format!("Failed to read rules file: {}", rules_path.display()));
            }
        };

        if text.is_empty() {
            return Ok(Self { codes, descr, rules });
        }

        for group in text.split("\n\n") {
            if group.starts_with("//") || group.trim().is_empty() {
                continue;
            }
            let sg: Vec<&str> = group.lines().collect();
            if sg.len() < 3 {
                eprintln!("WARNING: skipping malformed rules block (need 3-4 lines, got {}): {:?}", sg.len(), sg);
                continue;
            }

            let code: u64 = match sg[0].parse() {
                Ok(c) => c,
                Err(_) => {
                    eprintln!("WARNING: skipping rules block with invalid code '{}': {:?}", sg[0], sg);
                    continue;
                }
            };

            let mut item = ItemRule::new();
            item.code = code;
            item.desc = sg[1].to_owned();
            item.name = sg[2].to_owned();

            if sg.len() >= 4 {
                item.tags = split_tags(sg[3]);
            }

            if codes.contains_key(&item.code) || descr.contains_key(&item.desc) {
                eprintln!("WARNING: duplicate item found in rules list: [{:?}]", item);
            }
            codes.insert(item.code, rules.len());
            descr.insert(item.desc.clone(), rules.len());
            rules.push(item);
        }

        Ok(Self { codes, descr, rules })
    }
}

impl Purchases {
    pub fn init(purchases_path: &Path) -> Result<Self> {
        let text = match std::fs::read_to_string(purchases_path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Purchases(Vec::new()));
            }
            Err(e) => {
                return Err(e).with_context(|| format!("Failed to read purchases file: {}", purchases_path.display()));
            }
        };

        if text.trim().is_empty() {
            return Ok(Purchases(Vec::new()));
        }

        let mut v = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let parts: Vec<&str> = line.split("|").map(|s| s.trim()).collect();
            if parts.len() != 4 {
                eprintln!("WARNING: skipping malformed purchase line {} (expected 4 fields, got {}): {}", i + 1, parts.len(), line);
                continue;
            }

            let date: NaiveDate = match parts[0].parse() {
                Ok(d) => d,
                Err(_) => {
                    eprintln!("WARNING: skipping purchase line {} with bad date '{}': {}", i + 1, parts[0], line);
                    continue;
                }
            };
            let price: f64 = match parts[1].parse() {
                Ok(p) => p,
                Err(_) => {
                    eprintln!("WARNING: skipping purchase line {} with bad price '{}': {}", i + 1, parts[1], line);
                    continue;
                }
            };
            let name = parts[2].to_owned();
            let tags = split_tags(parts[3]);

            v.push(Purchase { price, name, tags, date, code: None });
        }

        Ok(Purchases(v))
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
    pub fn new(config: Config) -> Result<Self> {
        let maps = ItemMaps::init(&config.rules_file)?;
        let purchases = Purchases::init(&config.purchases_file)?;
        Ok(Self {
            config,
            maps,
            purchases,
            current_date: NaiveDate::from_ymd_opt(2001, 1, 1).unwrap(),
        })
    }

    pub fn set_date(&mut self, date: NaiveDate) {
        self.current_date = date;
    }

    pub fn process_purchase(&mut self, code: u64, desc: String, price: f64) {
        let idx = if self.maps.codes.contains_key(&code) {
            self.maps.codes[&code]
        } else if self.maps.descr.contains_key(&desc) {
            self.maps.descr[&desc]
        } else {
            println!("No item for code/desc/price: [{}]/[{}]/[{}]", code, desc, price);
            println!("Inserting entry into rules file");
            self.maps.codes.insert(code, self.maps.rules.len());
            self.maps.descr.insert(desc.clone(), self.maps.rules.len());
            self.maps.rules.push(ItemRule { code, desc, name: "UNKNOWN".to_owned(), tags: vec!["EXCLUDE".to_owned()] });
            self.maps.rules.len() - 1
        };

        self.purchases.push(Purchase {
            date: self.current_date,
            price,
            name: self.maps.rules[idx].name.clone(),
            tags: self.maps.rules[idx].tags.clone(),
            code: Some(code),
        });
    }

    pub fn purchases(&self) -> &Purchases {
        &self.purchases
    }

    pub fn get_max_lengths(&self) -> (usize, usize, usize) {
        let mut price_max = 0;
        let mut name_max = 0;
        let mut tags_max = 0;
        for p in &self.purchases.0 {
            price_max = max(price_max, p.price.to_string().len());
            name_max = max(name_max, p.name.len());
            let mut this_tags_len = max(0, p.tags.len() as i64 - 1 * 2) as usize;
            for tag in &p.tags {
                this_tags_len += tag.len();
            }
            tags_max = max(tags_max, this_tags_len);
        }
        (price_max, name_max, tags_max)
    }

    pub fn save_to_disk(&self) -> Result<()> {
        // Purchases File
        let (price_max, name_max, _tags_max) = self.get_max_lengths();
        let mut file = File::create(&self.config.purchases_file)
            .with_context(|| format!("Failed to create purchases file: {}", self.config.purchases_file.display()))?;
        for p in &self.purchases.0 {
            // Write receipt description for UNKNOWN items so they're identifiable
            let name = if p.name == "UNKNOWN" && p.code.is_some() {
                &self.maps.rules[self.maps.codes[&p.code.unwrap()]].desc
            } else {
                &p.name
            };

            let s = format!("{} | {:>price_max$.2} | {:<name_max$} | {}\n",
                p.date, p.price, name, p.tags.join(", "));
            file.write_all(s.as_bytes())?;
        }

        // Rules File
        let mut file = File::create(&self.config.rules_file)
            .with_context(|| format!("Failed to create rules file: {}", self.config.rules_file.display()))?;
        for r in &self.maps.rules {
            let mut s = format!("{}\n{}\n{}\n", r.code, r.desc, r.name);
            if !r.tags.is_empty() {
                s += &format!("{}\n", r.tags.join(", "));
            }
            s += "\n";
            file.write_all(s.as_bytes())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // split_tags tests
    #[test]
    fn test_split_tags_multiple() {
        assert_eq!(split_tags("a, b, c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_split_tags_single() {
        assert_eq!(split_tags("snacks"), vec!["snacks"]);
    }

    #[test]
    fn test_split_tags_empty() {
        assert_eq!(split_tags(""), vec![""]);
    }

    #[test]
    fn test_split_tags_whitespace() {
        assert_eq!(split_tags("  a , b ,c  "), vec!["a", "b", "c"]);
    }

    // Receipt::new store detection tests
    #[test]
    fn test_receipt_costco() {
        let r = Receipt::new("COSTCO WHOLESALE\nsome items".into());
        assert!(r.is_ok());
    }

    #[test]
    fn test_receipt_fredmeyer() {
        let r = Receipt::new("FRED MEYER\nsome items".into());
        assert!(r.is_ok());
    }

    #[test]
    fn test_receipt_winco() {
        let r = Receipt::new("WinCo Foods\nsome items".into());
        assert!(r.is_ok());
    }

    #[test]
    fn test_receipt_unknown_store() {
        let r = Receipt::new("RANDOM STORE\nsome items".into());
        assert!(r.is_err());
    }

    // Receipt::get_fields tests
    #[test]
    fn test_costco_normal_line() {
        let r = Receipt::new("costco wholesale".into()).unwrap();
        let result = r.get_fields("1234567 ORGANIC MILK 5.99");
        assert_eq!(result, Some((1234567, "ORGANIC MILK".into(), 5.99)));
    }

    #[test]
    fn test_costco_price_over_100() {
        let r = Receipt::new("costco wholesale".into()).unwrap();
        let result = r.get_fields("1234567 BIG PURCHASE 123.45");
        assert_eq!(result, Some((1234567, "BIG PURCHASE".into(), 123.45)));
    }

    #[test]
    fn test_costco_comma_price() {
        let r = Receipt::new("costco wholesale".into()).unwrap();
        let result = r.get_fields("1234567 ITEM NAME 5,99");
        assert_eq!(result, Some((1234567, "ITEM NAME".into(), 5.99)));
    }

    #[test]
    fn test_costco_no_match() {
        let r = Receipt::new("costco wholesale".into()).unwrap();
        let result = r.get_fields("just some random text");
        assert_eq!(result, None);
    }

    #[test]
    fn test_fredmeyer_normal_line() {
        let r = Receipt::new("fred meyer".into()).unwrap();
        let result = r.get_fields("12345 BREAD WHL WHT 3.49 F");
        assert_eq!(result, Some((12345, "BREAD WHL WHT".into(), 3.49)));
    }

    #[test]
    fn test_fredmeyer_different_tax_code() {
        let r = Receipt::new("fred meyer".into()).unwrap();
        let result = r.get_fields("12345 BREAD WHL WHT 3.49 T");
        assert_eq!(result, Some((12345, "BREAD WHL WHT".into(), 3.49)));
    }

    #[test]
    fn test_winco_normal_line() {
        let r = Receipt::new("winco".into()).unwrap();
        let result = r.get_fields("ONION YLW CO 4093 1.29");
        assert_eq!(result, Some((4093, "ONION YLW CO".into(), 1.29)));
    }

    #[test]
    fn test_winco_price_over_100() {
        let r = Receipt::new("winco".into()).unwrap();
        let result = r.get_fields("EXPENSIVE ITEM 9999 150.00");
        assert_eq!(result, Some((9999, "EXPENSIVE ITEM".into(), 150.00)));
    }

    // ItemMaps tests
    #[test]
    fn test_itemmaps_missing_file() {
        let path = Path::new("/tmp/itemizer_test_nonexistent_rules");
        let maps = ItemMaps::init(path).unwrap();
        assert!(maps.rules.is_empty());
    }

    #[test]
    fn test_itemmaps_valid_rules() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rules");
        std::fs::write(&path, "4093\nONION YLW CO\nOnions\nveggies, produce\n\n1326\nCOCONUT STRIPS\nCoconut Strips\nsnacks\n").unwrap();

        let maps = ItemMaps::init(&path).unwrap();
        assert_eq!(maps.rules.len(), 2);
        assert_eq!(maps.rules[0].name, "Onions");
        assert_eq!(maps.rules[0].tags, vec!["veggies", "produce"]);
        assert_eq!(maps.rules[1].name, "Coconut Strips");
        assert!(maps.codes.contains_key(&4093));
        assert!(maps.descr.contains_key("ONION YLW CO"));
    }

    #[test]
    fn test_itemmaps_skips_malformed_block() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rules");
        // First block is malformed (only 1 line), second is valid
        std::fs::write(&path, "bad_block\n\n4093\nONION YLW CO\nOnions\n").unwrap();

        let maps = ItemMaps::init(&path).unwrap();
        assert_eq!(maps.rules.len(), 1);
        assert_eq!(maps.rules[0].name, "Onions");
    }

    #[test]
    fn test_itemmaps_skips_bad_code() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rules");
        std::fs::write(&path, "not_a_number\nDESC\nName\n").unwrap();

        let maps = ItemMaps::init(&path).unwrap();
        assert!(maps.rules.is_empty());
    }

    #[test]
    fn test_itemmaps_skips_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rules");
        std::fs::write(&path, "// This is a comment\n\n4093\nONION YLW CO\nOnions\n").unwrap();

        let maps = ItemMaps::init(&path).unwrap();
        assert_eq!(maps.rules.len(), 1);
    }

    // Purchases tests
    #[test]
    fn test_purchases_missing_file() {
        let path = Path::new("/tmp/itemizer_test_nonexistent_purchases");
        let purchases = Purchases::init(path).unwrap();
        assert!(purchases.is_empty());
    }

    #[test]
    fn test_purchases_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("purchases");
        std::fs::write(&path, "").unwrap();

        let purchases = Purchases::init(&path).unwrap();
        assert!(purchases.is_empty());
    }

    #[test]
    fn test_purchases_valid_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("purchases");
        std::fs::write(&path, "2024-07-21 | 5.99 | Onions | veggies, produce\n").unwrap();

        let purchases = Purchases::init(&path).unwrap();
        assert_eq!(purchases.len(), 1);
        assert_eq!(purchases[0].name, "Onions");
        assert_eq!(purchases[0].price, 5.99);
        assert_eq!(purchases[0].tags, vec!["veggies", "produce"]);
    }

    #[test]
    fn test_purchases_skips_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("purchases");
        std::fs::write(&path, "bad line\n2024-07-21 | 5.99 | Onions | veggies\n").unwrap();

        let purchases = Purchases::init(&path).unwrap();
        assert_eq!(purchases.len(), 1);
    }

    // Round-trip test
    #[test]
    fn test_save_and_reload_purchases() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            image_dir: dir.path().join("images"),
            upscaled_image_dir: dir.path().join("upscaled"),
            done_file: dir.path().join("done"),
            rules_file: dir.path().join("rules"),
            purchases_file: dir.path().join("purchases"),
        };

        // Create empty rules and purchases files
        std::fs::write(&config.rules_file, "4093\nONION YLW CO\nOnions\nveggies\n").unwrap();
        std::fs::write(&config.purchases_file, "").unwrap();

        let mut itemizer = FileItemizer::new(config).unwrap();
        itemizer.set_date(NaiveDate::from_ymd_opt(2024, 7, 21).unwrap());
        itemizer.process_purchase(4093, "ONION YLW CO".into(), 5.99);

        itemizer.save_to_disk().unwrap();

        // Reload and verify
        let purchases = Purchases::init(&itemizer.config.purchases_file).unwrap();
        assert_eq!(purchases.len(), 1);
        assert_eq!(purchases[0].name, "Onions");
        assert_eq!(purchases[0].price, 5.99);
    }

    // image_done tests
    #[test]
    fn test_image_done_missing_file() {
        let result = image_done("test.jpg", Path::new("/tmp/itemizer_nonexistent_done"));
        assert_eq!(result.unwrap(), false);
    }

    #[test]
    fn test_image_done_found() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("done");
        std::fs::write(&path, "img1.jpg\nimg2.jpg\n").unwrap();

        assert_eq!(image_done("img1.jpg", &path).unwrap(), true);
        assert_eq!(image_done("img3.jpg", &path).unwrap(), false);
    }
}
