mod abort;
mod cb;
mod config;
mod err;
mod sc;
mod stream;
mod util;
use std::{thread, time::Duration};

fn main() {
    let filename = "cb-config.json";
    let mut models = config::load(filename).unwrap();
    while !abort::get().unwrap() {
        models.download().unwrap();
        for _ in 0..300 {
            thread::sleep(Duration::from_millis(200));
            if abort::get().unwrap() {
                break;
            }
            models.update_config().unwrap();
        }
    }
}
