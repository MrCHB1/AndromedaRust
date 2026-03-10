use std::{fmt::{Debug, Display}, fs::{File, OpenOptions, create_dir_all}, io::Write, path::Path, sync::Mutex};

use chrono::Local;
use once_cell::sync::Lazy;

pub struct Debugger;

static LOG_FILE: Lazy<Mutex<Option<File>>> = Lazy::new(|| Mutex::new(None));
const DEBUGGING_ENABLED: bool = true;

impl Debugger {
    pub fn init_log_file(path: &str) {
        if !DEBUGGING_ENABLED { 
            println!("Debugging has been disabled, did not make a log file.");
            return;
        }

        let p = Path::new(path);

        if let Some(parent) = p.parent() {
            create_dir_all(parent).unwrap();
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .unwrap();

        let mut log_file = LOG_FILE.lock().unwrap();
        *log_file = Some(file);
    }

    fn write_file(message: &str) {
        let mut log_file = LOG_FILE.lock().unwrap();

        if let Some(file) = log_file.as_mut() {
            let _ = writeln!(file, "{}", message);
        }
    }

    pub fn log<T: Debug + Display>(message: T) {
        if !DEBUGGING_ENABLED { return; }

        let local_time = Local::now();
        let time_fmt = local_time.format("%m/%d/%Y @ %H:%M:%S");
        let msg = format!("[{}] {}", time_fmt, message);

        println!("{}", msg);
        Self::write_file(&msg);
    }

    pub fn log_notime<T: Debug + Display>(message: T) {
        if !DEBUGGING_ENABLED { return; }

        println!("{}", message);
        Self::write_file(&format!("{}", &message));
    }

    pub fn log_warning<T: Debug + Display>(message: T) {
        if !DEBUGGING_ENABLED { return; }

        let local_time = Local::now();
        let time_fmt = local_time.format("%m/%d/%Y @ %H:%M:%S");
        let msg = format!("[{}] {}", time_fmt, message);
        
        println!("[{}] \x1b[33m(WARNING) {}\x1b[0m", time_fmt, message);
        Self::write_file(&msg);
    }

    pub fn log_error<T: Debug + Display>(message: T) {
        if !DEBUGGING_ENABLED { return; }

        let local_time = Local::now();
        let time_fmt = local_time.format("%m/%d/%Y @ %H:%M:%S");
        let msg = format!("[{}] {}", time_fmt, message);
        
        println!("[{}] \x1b[31m(ERROR) {}\x1b[0m", time_fmt, message);
        Self::write_file(&msg);
    }
}