use std::fmt::{Debug, Display};

use chrono::Local;

pub struct Debugger;

impl Debugger {
    pub fn log<T: Debug + Display>(message: T) {
        let local_time = Local::now();
        println!("[{}] {}", local_time.format("%m/%d/%Y @ %H:%M:%S"), message);
    }

    pub fn log_warning<T: Debug + Display>(message: T) {
        let local_time = Local::now();
        
    }
}