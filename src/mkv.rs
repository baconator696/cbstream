use crate::{e, h, o, s};
use crate::{stream, util};
use std::sync::{Arc, RwLock};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
type Hresult<T> = result::Result<T, String>;
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
    // starts mkvmerge process
    let mut child = process::Command::new(mkv_exists().map_err(s!())?)
        .arg(format!("@{}", json_file.str()))
        .arg("-q")
        .stderr(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .spawn()
        .map_err(e!())?;
    // read from stderr/stdout pipes
    use io::Read;
    let mut stdout = child.stdout.take().ok_or_else(o!())?;
    let mut stderr = child.stderr.take().ok_or_else(o!())?;
    let stdout_handle = thread::spawn(move || -> Hresult<String> {
        let mut out = String::new();
        stdout.read_to_string(&mut out).map_err(e!())?;
        Ok(out)
    });
    let stderr_handle = thread::spawn(move || -> Hresult<String> {
        let mut out = String::new();
        stderr.read_to_string(&mut out).map_err(e!())?;
        Ok(out)
    });
    // monitors system memory
    let mut sys = sysinfo::System::new_all();
    let exit_status = loop {
        match child.try_wait().map_err(e!())? {
            Some(o) => break o,
            None => (),
        }
        sys.refresh_memory();
        if sys.available_memory() < 200000000 {
            child.kill().map_err(e!())?;
            let output_filepath = format!("{}.mkv", filepath);
            if fs::metadata(&output_filepath).is_ok() {
                fs::remove_file(output_filepath).map_err(e!())?;
            }
            return Err("not enough memory, killed mkvmerge").map_err(s!())?;
        }
        thread::sleep(time::Duration::from_millis(200));
    };
    let stdout = stdout_handle.join().map_err(h!())?.map_err(s!())?;
    let stderr = stderr_handle.join().map_err(h!())?.map_err(s!())?;
    // processes output
    if exit_status.code().ok_or_else(o!())? == 2 {
        return Err(format!("{}{}", stdout.trim(), stderr.trim())).map_err(s!())?;
    }
    return Ok(());
}
