use unshell_lib::error;

#[derive(Clone)]
pub struct Database {
    db: sled::Db,
}

impl Database {
    pub fn new(database: String) -> Self {
        Self {
            db: sled::open(database).expect("Failed to open database"),
        }
    }

    pub fn put_value(&self, key: &str, value: &str) -> Result<(), String> {
        match self.db.insert(key, value) {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to load '{}' from database: {}", key, e);
                Err(e.to_string())
            }
        }
    }

    pub fn get_value(&self, key: &str) -> Result<String, String> {
        match self.db.get(key) {
            Ok(v) => match v {
                Some(v) => Ok(String::from_utf8_lossy(&v.to_vec()).to_string()),
                None => Err(format!("Could not find key '{}'", key)),
            },
            Err(e) => {
                error!("Failed to load '{}' from database: {}", key, e);
                Err(e.to_string())
            }
        }
    }
}
