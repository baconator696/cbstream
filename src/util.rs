use crate::{e, s};
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
        thread::sleep(time::Duration::from_millis(250));
    }
    r
}
pub fn get_retry_vec(url: &str, retry: i32) -> Result<Vec<u8>> {
    let f = |url| {
        let resp = reqwest::blocking::get(url).map_err(e!())?;
        let resp_code = resp.status();
        if resp_code != 200 {
            return Err(format!("{}|{}", resp.text().map_err(e!())?.trim(), resp_code)).map_err(s!())?;
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
            if e.kind() != io::ErrorKind::AlreadyExists {
                return Err(e).map_err(e!())?;
            }
        }
        _ => (),
    }
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
/// json file management
pub struct JsonFile(String);
impl JsonFile {
    pub fn new(filepath: String, contents: String) -> Result<Self> {
        fs::write(&filepath, contents).map_err(e!())?;
        Ok(JsonFile(filepath))
    }
    pub fn str(&self) -> &str {
        &self.0
    }
}
impl Drop for JsonFile {
    fn drop(&mut self) {
        _ = fs::remove_file(&self.0);
    }
}
/// checks if mkvtoolnix is installed and returns path of mkvmuxer
pub fn mkv_exists() -> Result<String> {
    let mkv_path = if cfg!(target_os = "windows") {
        mkv_exists_windows().map_err(s!())?
    } else {
        "/bin/mkvmerge".to_string()
    };
    if !fs::metadata(&mkv_path).is_ok() {
        return Err(format!("can't find {}", mkv_path))?;
    }
    Ok(mkv_path)
}
#[cfg(not(windows))]
fn mkv_exists_windows() -> Result<String> {
    Ok(String::new())
}
#[cfg(windows)]
fn mkv_exists_windows() -> Result<String> {
    use crate::o;
    use winreg::RegKey;
    use winreg::enums::*;
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let uninstall_paths = [
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\MKVToolNix",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\MKVToolNix",
    ];
    for path in uninstall_paths {
        let mkv_key = match hklm.open_subkey_with_flags(path, KEY_READ | KEY_WOW64_64KEY) {
            Ok(r) => r,
            _ => continue,
        };
        let uninstall_string: String = mkv_key.get_value("UninstallString").map_err(e!())?;
        let dir_split = uninstall_string.split("\\").collect::<Vec<&str>>();
        let dir = dir_split.get(..dir_split.len() - 1).ok_or_else(o!())?.join("\\");
        let path = format!("{}\\mkvmerge.exe", dir);
        return Ok(path);
    }
    Ok("C:\\Program Files\\MKVToolNix\\mkvmerge.exe".to_string())
}
