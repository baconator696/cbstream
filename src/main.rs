mod abort;
mod config;
mod err;
mod muxer;
mod platforms;
mod stream;
mod util;
use std::{path::Path, thread, time::Duration};

fn main() {
    const TAG: Option<&str> = option_env!("TAG");
    println!("cbstream {}", TAG.unwrap_or_default());
    let filename = Path::new("cb-config.json");
    let mut models = config::init(&filename).unwrap();
    while !abort::get().unwrap() {
        models.download().unwrap();
        for _ in 0..60 {
            thread::sleep(Duration::from_secs(1));
            if abort::get().unwrap() {
                break;
            }
            models.update_config().unwrap();
        }
    }
}
