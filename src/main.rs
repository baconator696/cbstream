mod abort;
mod cb;
mod config;
mod err;
mod mfc;
mod sc;
mod scvr;
mod stream;
mod util;
use std::{thread, time::Duration, *};

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
            match models.update_config().map_err(s!()) {
                Err(e) => eprintln!("{}", e),
                Ok(r) => r,
            };
        }
    }
}
