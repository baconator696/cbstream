use crate::{e, s};
use std::{io::Read, *};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
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
            return Err(format!("{}", resp_code)).map_err(s!())?;
        }
        Ok(resp_text)
    };
    let mut r = Err("".into());
    for _ in 0..retry {
        r = f(url);
        if r.is_ok() {
            break;
        }
    }
    r
}
pub fn get_retry_vec(url: &str, retry: i32) -> Result<Vec<u8>> {
    let f = |url| {
        let resp = reqwest::blocking::get(url).map_err(e!())?;
        let resp_code = resp.status();
        if resp_code != 200 {
            return Err(format!("{}|{}", resp.text().map_err(e!())?, resp_code)).map_err(s!())?;
        }
        Ok(resp.bytes().map_err(e!())?.to_vec())
    };
    let mut r = Err("".into());
    for _ in 0..retry {
        r = f(url);
        if r.is_ok() {
            break;
        }
    }
    r
}
