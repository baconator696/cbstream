use crate::{e, o, s};
use crate::{stream, util};
use std::sync::{Arc, RwLock};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
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
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\MKVToolNix",
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\MKVToolNix",
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
pub fn mkvmerge(streams: &Vec<Arc<RwLock<stream::Stream>>>, filepath: &str, filename: &str) -> Result<()> {
    // creates arg list for mkvmerge
    let mut arg_list: Vec<String> = Vec::with_capacity(streams.len() * 2 + 2);
    arg_list.push("-o".into());
    arg_list.push(format!("{}.mkv", filepath));
    for stream in streams {
        let s = &(*stream.write().map_err(s!())?);
        if s.file.is_some() {
            arg_list.push(s.filepath.clone());
            arg_list.push("+".into());
        }
    }
    arg_list.pop();
    let json = serde_json::to_string(&arg_list).map_err(e!())?;
    let json_filename = format!("{}{}.json", util::temp_dir().map_err(s!())?, filename);
    let json_file = JsonFile::new(json_filename, json).map_err(s!())?;
    // starts mkvmerge process and monitors system memory
    let mut child = process::Command::new(mkv_exists().map_err(s!())?)
        .arg(format!("@{}", json_file.str()))
        .stderr(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .spawn()
        .map_err(e!())?;
    let mut sys = sysinfo::System::new_all();
    while child.try_wait().map_err(e!())?.is_none() {
        sys.refresh_memory();
        if sys.available_memory() < 200000000 {
            child.kill().map_err(e!())?;
            return Err("not enough memory, killed mkvmerge".into());
        }
        thread::sleep(time::Duration::from_millis(200));
    }
    // processes output
    let output = child.wait_with_output().map_err(e!())?;
    if output.status.code().ok_or_else(o!())? == 2 {
        let e = format!(
            "{}:{}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return Err(format!("{}", e)).map_err(s!())?;
    }
    return Ok(());
}
