mod abort;
mod config;
mod err;
mod muxer;
mod platforms;
mod stream;
mod util;
use std::{
    path::{PathBuf},
    time::Duration,
    *,
};

fn main() {
    const TAG: Option<&str> = option_env!("TAG");
    println!("cbstream {}", TAG.unwrap_or_default());

    let filename = PathBuf::from(
        env::args_os()
            .nth(1)
            .or_else(|| env::var_os("CONFIG"))
            .unwrap_or_else(|| "cb-config.json".into()),
    );

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
