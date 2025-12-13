use chrono::Local;
// use lazy_static::lazy_static;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

// --- Constants ---
/// The directory where log files will be stored.
const LOG_DIR: &str = "logs";

/// The maximum number of logs to return in one call to `poll_logs`.
const LOG_COUNT: usize = 10;

/// The full path to the log file.
/// Initialized once based on the startup time.
#[static_init::dynamic]
static LOG_FILE_PATH: PathBuf = {
    // 1. Determine the log directory path
    let log_dir_path = PathBuf::from(LOG_DIR);

    // 2. Create the log directory if it does not exist
    if let Err(e) = fs::create_dir_all(&log_dir_path) {
        eprintln!("Error creating log directory {:?}: {}", log_dir_path, e);
        // Panic or handle error as appropriate for your application's needs
        // Panicking here to ensure the logger can't be used if the path is invalid/unwritable
        panic!("Failed to initialize log directory.");
    }

    // 3. Generate the unique filename based on the current local time
    let now = Local::now();
    let filename = format!("{}.log", now.format("%Y%m%d_%H%M%S"));

    // 4. Combine the directory path and the filename
    log_dir_path.join(filename)
};

/// A static utility module for logging operations.
pub struct Logger;

impl Logger {
    /// Writes a log entry to the current log file.
    ///
    /// The log entry includes a timestamp and the provided message.
    ///
    /// # Arguments
    ///
    /// * `message` - The string content of the log entry.
    pub fn log(message: String) {
        // 1. Format the complete log line with timestamp
        let now = Local::now();
        let log_line = format!("[{}] {}\n", now.format("%Y-%m-%d %H:%M:%S%.3f"), message);

        // 2. Open the file in append mode, creating it if it doesn't exist.
        // The file path is guaranteed to be valid due to the `lazy_static` initialization.
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

    /// Reads and returns the most recent logs from the file.
    ///
    /// The total number of logs returned is limited by `LOG_COUNT`.
    /// The `offset` determines how far back in history to start reading.
    ///
    /// # Arguments
    ///
    /// * `offset` - The number of most recent logs to skip.
    ///
    /// # Returns
    ///
    /// A `Vec<String>` containing the logs, or an empty `Vec` on failure.
    pub fn poll_logs(offset: usize) -> Vec<String> {
        // Array of String is not a idiomatic return type in Rust,
        // so we return a Vector, which serves the same purpose.
        // The size constraint (LOG_COUNT) is applied internally.
        match File::open(&*LOG_FILE_PATH) {
            Ok(file) => {
                let reader = BufReader::new(file);

                // Collect all lines into a vector
                let lines: Vec<String> = reader
                    .lines()
                    .filter_map(|line| line.ok()) // Ignore lines that fail to read
                    .collect();

                // Determine the starting index for the slice (from the end of the vector)
                // This logic correctly handles the offset and LOG_COUNT limits.

                let total_lines = lines.len();
                if offset >= total_lines {
                    // Offset is past the beginning of the file, return nothing
                    return Vec::new();
                }

                // Start from the end, minus the offset, minus the number of logs to read.
                // We use checked subtraction to prevent panic if it would result in a negative number.
                let start_index = total_lines
                    .checked_sub(offset)
                    .and_then(|i| i.checked_sub(LOG_COUNT))
                    .unwrap_or(0); // If either subtraction fails, start from 0

                // End index is determined by subtracting the offset from the total length.
                let end_index = total_lines.checked_sub(offset).unwrap_or(total_lines);

                // Get the slice of the lines
                let slice = &lines[start_index..end_index];

                // The logs are currently in historical order (oldest to newest within the slice).
                // We must reverse them to return the "most recent" logs in descending order.
                slice.iter().rev().cloned().collect()
            }
            Err(e) => {
                // Return an empty vector and print an error message if the file cannot be read
                eprintln!("Error reading log file {:?}: {}", *LOG_FILE_PATH, e);
                Vec::new()
            }
        }
    }
}

// --- Example Usage ---

// fn main() {
//     // Write some logs
//     Logger::log("Application started.".to_string());
//     Logger::log("Configuration loaded.".to_string());

//     for i in 1..=20 {
//         Logger::log(format!("Processing request #{}", i));
//     }

//     Logger::log("Task completed.".to_string());

//     // --- Test 1: Get the 10 most recent logs (offset 0) ---
//     println!("--- Most Recent Logs (LOG_COUNT={}) ---", LOG_COUNT);
//     let recent_logs = Logger::poll_logs(0);
//     for log in &recent_logs {
//         println!("{}", log);
//     }

//     // The output should be:
//     // [timestamp] Task completed.
//     // [timestamp] Processing request #20
//     // [timestamp] Processing request #19
//     // ...
//     // [timestamp] Processing request #12

//     println!(
//         "\n--- Logs from the past (Offset 15, LOG_COUNT={}) ---",
//         LOG_COUNT
//     );
//     // --- Test 2: Skip the 15 most recent logs, then get the next 10 ---
//     let historical_logs = Logger::poll_logs(15);
//     for log in &historical_logs {
//         println!("{}", log);
//     }

//     // The output should be:
//     // [timestamp] Processing request #6
//     // [timestamp] Processing request #5
//     // ...
//     // [timestamp] Application started.

//     println!("\n--- Testing a high offset (Offset 100) ---");
//     // --- Test 3: Test an offset that goes beyond the log file length ---
//     let empty_logs = Logger::poll_logs(100);
//     if empty_logs.is_empty() {
//         println!("Correctly returned empty set for high offset.");
//     }
// }
