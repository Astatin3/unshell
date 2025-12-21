use axum::extract::{Path, State};
use axum::{Extension, Json};
use chrono::Local;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use unshell_lib::debug;

use crate::Server;
use crate::auth::structs::CurrentUser;

const LOG_DIR: &str = "logs";

const LOG_COUNT: usize = 100;

/// The full path to the log file.
/// Initialized once based on the startup time.
#[static_init::dynamic]
static LOG_FILE_PATH: PathBuf = {
    let log_dir_path = PathBuf::from(LOG_DIR);

    if let Err(e) = fs::create_dir_all(&log_dir_path) {
        eprintln!("Error creating log directory {:?}: {}", log_dir_path, e);

        panic!("Failed to initialize log directory.");
    }

    let now = Local::now();
    let filename = format!("{}.log", now.format("%Y%m%d_%H%M%S"));

    log_dir_path.join(filename)
};

/// A static utility module for logging operations.
pub struct Logger;

impl Logger {
    pub fn log(message: String) {
        let log_line = format!("{}\n", message);

        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&*LOG_FILE_PATH)
        {
            Ok(mut file) => {
                // 3. Write the log line to the file
                if let Err(e) = file.write_all(log_line.as_bytes()) {
                    eprintln!("Error writing log to file {:?}: {}", *LOG_FILE_PATH, e);
                }
            }
            Err(e) => {
                eprintln!("Error opening log file {:?}: {}", *LOG_FILE_PATH, e);
            }
        }
    }

    pub fn poll_logs(offset: usize) -> Vec<String> {
        match File::open(&*LOG_FILE_PATH) {
            Ok(file) => {
                let reader = BufReader::new(file);
                let lines: Vec<String> = reader
                    .lines()
                    .filter_map(|line| line.ok()) // Ignore lines that fail to read
                    .collect();

                let total_lines = lines.len();
                if offset >= total_lines {
                    return Vec::new();
                }

                let start_index = total_lines
                    .checked_sub(offset)
                    .and_then(|i| i.checked_sub(LOG_COUNT))
                    .unwrap_or(0);

                let end_index = total_lines.checked_sub(offset).unwrap_or(total_lines);

                let slice = &lines[start_index..end_index];

                slice.iter().cloned().collect()
            }
            Err(e) => {
                eprintln!("Error reading log file {:?}: {}", *LOG_FILE_PATH, e);
                Vec::new()
            }
        }
    }

    pub async fn poll_logs_api(
        State(_): State<Server>,
        Extension(_): Extension<CurrentUser>,
        Path(offset): Path<usize>,
    ) -> axum::Json<serde_json::Value> {
        debug!("GET /api/log/{}", offset);
        let result = Self::poll_logs(offset);

        Json(serde_json::to_value(result).unwrap())
    }
}
