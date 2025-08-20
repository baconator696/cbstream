use crate::{e, o, s};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, RwLock},
    *,
};
type Result<T> = result::Result<T, Box<dyn error::Error>>;

static USERAGENT: OnceLock<Arc<RwLock<String>>> = OnceLock::new();
pub fn set_useragent(useragent: String) -> Result<()> {
    let u = USERAGENT.get_or_init(|| Arc::new(RwLock::new(String::new())));
    *u.write().map_err(s!())? = useragent;
    Ok(())
}
pub fn get_useragent() -> Result<String> {
    let u = USERAGENT.get().ok_or_else(o!())?;
    Ok((*u.read().map_err(s!())?).clone())
}

pub fn get_retry(url: &str, retry: i32, headers: Option<&HashMap<String, String>>) -> Result<String> {
    let f = || {
        let client = reqwest::blocking::Client::new();
        let build = if let Some(headers) = headers {
            let mut h = reqwest::header::HeaderMap::new();
            use reqwest::header::HeaderName;
            use reqwest::header::HeaderValue;
            use std::str::FromStr;
            for (k, v) in headers {
                h.insert(HeaderName::from_str(&k).map_err(e!())?, HeaderValue::from_str(&v).map_err(e!())?);
            }
            client.get(url).headers(h)
        } else {
            client.get(url)
        };
        let resp = build.send().map_err(e!())?;
        if resp.status() != 200 {
            return Err(format!("{}-{}", resp.status(), resp.text().map_err(e!())?))?;
        }
        Ok(resp.text().map_err(e!())?)
    };
    let mut r = Err("".into());
    for _ in 0..retry {
        r = f();
        if r.is_ok() {
            break;
        }
        thread::sleep(time::Duration::from_millis(250));
    }
    r
}
pub fn get_retry_vec(url: &str, retry: i32, headers: Option<&HashMap<String, String>>) -> Result<Vec<u8>> {
    let f = |url| {
        let client = reqwest::blocking::Client::new();
        let build = if let Some(headers) = headers {
            let mut h = reqwest::header::HeaderMap::new();
            use reqwest::header::HeaderName;
            use reqwest::header::HeaderValue;
            use std::str::FromStr;
            for (k, v) in headers {
                h.insert(HeaderName::from_str(&k).map_err(e!())?, HeaderValue::from_str(&v).map_err(e!())?);
            }
            client.get(url).headers(h)
        } else {
            client.get(url)
        };
        let resp = build.send().map_err(e!())?;
        if resp.status() != 200 {
            return Err(format!("{}-{}", resp.status(), resp.text().map_err(e!())?))?;
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
pub fn post_retry(url: &str, retry: i32, headers: Option<&HashMap<String, String>>, payload: &str, content_type: &str) -> Result<String> {
    let f = || {
        let client = reqwest::blocking::Client::new();
        let build = if let Some(headers) = headers {
            let mut h = reqwest::header::HeaderMap::new();
            use reqwest::header::HeaderName;
            use reqwest::header::HeaderValue;
            use std::str::FromStr;
            for (k, v) in headers {
                h.insert(HeaderName::from_str(&k).map_err(e!())?, HeaderValue::from_str(&v).map_err(e!())?);
            }
            client.post(url).headers(h)
        } else {
            client.post(url)
        };
        let resp = build
            .body(payload.to_string())
            .header("content-type", content_type)
            .send()
            .map_err(e!())?;
        if resp.status() != 200 {
            return Err(format!("{}-{}", resp.status(), resp.text().map_err(e!())?))?;
        }
        Ok(resp.text().map_err(e!())?)
    };
    let mut r = Err("".into());
    for _ in 0..retry {
        r = f();
        if r.is_ok() {
            break;
        }
        thread::sleep(time::Duration::from_millis(250));
    }
    r
}
pub fn create_headers(json_map: serde_json::Value) -> Result<HashMap<String, String>> {
    let mut headers: HashMap<String, String> = HashMap::new();
    for (k, v) in json_map.as_object().ok_or_else(o!())? {
        headers.insert(k.clone(), v.as_str().ok_or_else(o!())?.to_string());
    }
    Ok(headers)
}
/// returns current date and time in "24-02-29_23-12" format
pub fn date() -> String {
    let now = chrono::Local::now();
    now.format("%y-%m-%d_%H-%M").to_string()
}
/// returns temp directory location ex. "/tmp/cbstream/""
pub fn temp_dir() -> Result<PathBuf> {
    let mut temp_dir = PathBuf::new();
    if cfg!(target_os = "windows") {
        let t = env::var("TEMP").map_err(e!())?;
        temp_dir.push(t);
    } else {
        let t = match env::var("TEMP") {
            Ok(r) => r,
            _ => format!("/tmp"),
        };
        temp_dir.push(t);
    };
    temp_dir.push("cbstream");
    Ok(temp_dir)
}
pub fn create_dir(dir: &Path) -> Result<()> {
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
pub fn url_prefix(url: &str) -> Option<&str> {
    let n = url.rfind("/")?;
    url.get(..n)
}
pub fn remove_non_num(url: &str) -> String {
    url.chars().filter(|c| c.is_ascii_digit()).collect::<String>()
}
