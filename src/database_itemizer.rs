use rusqlite::{params, Connection};

pub struct DatabaseItemizer {
    pub db: Connection,
    pub date: NaiveDate,
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

    fn save_to_disk(&self) {}
}
