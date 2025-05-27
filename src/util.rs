use crate::e;
use std::{io::Read, *};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub const SLASH: &str = if cfg!(target_os = "windows") { "\\" } else { "/" };
pub fn get_retry(url: &str, retry: i32) -> Result<String> {
    let f = |url| {
        let resp = reqwest::blocking::get(url).map_err(e!())?;
        let resp_code = resp.status();
        let resp_text = if let Some(v) = resp.headers().get("content-encoding") {
            if v.to_str().map_err(e!())? == "gzip" {
                let resp_data = resp.bytes().map_err(e!())?.to_vec();
                let mut resp_text = String::new();
                let mut decoder = flate2::read::GzDecoder::new(resp_data.as_slice());
                decoder.read_to_string(&mut resp_text).map_err(e!())?;
                resp_text
            } else {
                resp.text().map_err(e!())?
            }
        } else {
            resp.text().map_err(e!())?
        };
        if resp_code != 200 {
            return Err(format!("{}", resp_code))?;
        }
        Ok(resp_text)
    };
    let mut r = Err("".into());
    for _ in 0..retry {
        r = f(url);
        if r.is_ok() {
            break;
        }
        thread::sleep(time::Duration::from_millis(250));
    }
    r
}
pub fn get_retry_vec(url: &str, retry: i32) -> Result<Vec<u8>> {
    let f = |url| {
        let resp = reqwest::blocking::get(url).map_err(e!())?;
        let resp_code = resp.status();
        if resp_code != 200 {
            return Err(format!("{}|{}", resp.text().map_err(e!())?.trim(), resp_code))?;
        }
        Ok(resp.bytes().map_err(e!())?.to_vec())
    };
    let mut r = Err("".into());
    for _ in 0..retry {
        r = f(url);
        if r.is_ok() {
            break;
        }
        thread::sleep(time::Duration::from_millis(250));
    }
    r
}
// returns string in between two strings and returns last index where found
pub fn _find<'a>(search: &'a str, start: &str, end: &str, i: usize) -> Option<(&'a str, usize)> {
    let start_loc = search.get(i..)?.find(start)? + i + start.len();
    let end_loc = search.get(start_loc..)?.find(end)? + start_loc;
    let find = search.get(start_loc..end_loc)?;
    let offset = end_loc + end.len();
    Some((find, offset))
}
/// returns current date and time in "24-02-29_23-12" format
pub fn date() -> String {
    let now = chrono::Local::now();
    now.format("%y-%m-%d_%H-%M").to_string()
}
/// returns temp directory location ex. "/tmp/cbstream/""
pub fn temp_dir() -> Result<String> {
    let temp_dir = if cfg!(target_os = "windows") {
        let t = env::var("TEMP").map_err(e!())?;
        format!("{}\\cbstream\\", t)
    } else {
        let t = match env::var("TEMP") {
            Ok(r) => r,
            _ => format!("/tmp"),
        };
        format!("{}/cbstream/", t)
    };
    Ok(temp_dir)
}
// creates temp directory
pub fn create_dir(dir: &str) -> Result<()> {
    match fs::create_dir_all(dir) {
        Err(e) => {
            if e.kind() == io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(e)
            }
        }
        Ok(r) => Ok(r),
    }
    .map_err(e!())?;
    Ok(())
}
// returns url prefix
pub fn url_prefix(url: &str) -> Option<&str> {
    let mut n = 0;
    loop {
        n = match str::find(url.get(n + 1..)?, "/") {
            Some(m) => m + n + 1,
            None => break,
        };
    }
    url.get(..n)
}
pub fn remove_non_num(url: &str) -> String {
    url.chars().filter(|c| c.is_ascii_digit()).collect::<String>()
}
