use std::sync::atomic::{AtomicBool, AtomicU16};
use std::sync::Mutex;

pub static SHARED_KEY: Mutex<String> = Mutex::new(String::new());
pub static SESSION_NEW_KEY: Mutex<String> = Mutex::new(String::new());
pub static SERVER_RUNNING: AtomicBool = AtomicBool::new(false);
pub static SERVER_PORT: AtomicU16 = AtomicU16::new(8080);
