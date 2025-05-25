use crate::{e, h, o, s};
use crate::{platform, stream, util};
use std::io::{Read, Write};
use std::sync::{Arc, RwLock};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
type Hresult<T> = result::Result<T, String>;
/// json file management
pub struct FileManage(String);
impl FileManage {
    pub fn new(filepath: String) -> Result<Self> {
        Ok(Self(filepath))
    }
    pub fn filepath(&self) -> &str {
        &self.0
    }
}
impl Drop for FileManage {
    fn drop(&mut self) {
        _ = fs::remove_file(&self.0);
    }
}
/// checks if mkvtoolnix is installed and returns path of mkvmuxer
fn mkv_exists() -> Result<Option<String>> {
    let path = if cfg!(target_os = "windows") {
        let path = mkv_exists_windows().map_err(s!())?;
        if fs::metadata(&path).is_ok() { Some(path) } else { None }
    } else {
        let path = "mkvmerge";
        match process::Command::new(path).arg("-V").output() {
            Ok(_) => Some(path.to_string()),
            Err(_) => None,
        }
    };
    Ok(path)
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
fn ffmpeg_exists() -> Result<Option<&'static str>> {
    let path = "ffmpeg";
    match process::Command::new(path).arg("-version").output() {
        Ok(_) => Ok(Some(path)),
        Err(_) => Ok(None),
    }
}
fn mkvmerge(mkvmerge_path: &str, streams: &Vec<Arc<RwLock<stream::Stream>>>, filepath: &str, filename: &str) -> Result<()> {
    let filepath = format!("{}.mkv", filepath);
    // creates arg list for mkvmerge
    let mut arg_list: Vec<String> = Vec::with_capacity(streams.len() * 2 + 2);
    arg_list.push("-o".into());
    arg_list.push(filepath.clone());
    for stream in streams {
        let s = &(*stream.write().map_err(s!())?);
        if s.file.is_some() {
            arg_list.push(s.filepath.clone());
            arg_list.push("+".into());
        }
    }
    arg_list.pop();
    let json = serde_json::to_string(&arg_list).map_err(e!())?;
    let json_filepath = format!("{}{}.json", util::temp_dir().map_err(s!())?, filename);
    fs::write(&json_filepath, json).map_err(e!())?;
    let json_file = FileManage::new(json_filepath).map_err(s!())?;
    // starts mkvmerge process
    let mut child = process::Command::new(mkvmerge_path)
        .arg(format!("@{}", json_file.filepath()))
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
            if fs::metadata(&filepath).is_ok() {
                fs::remove_file(filepath).map_err(e!())?;
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
fn ffmpeg(ffmpeg_path: &str, streams: &Vec<Arc<RwLock<stream::Stream>>>, filepath: &str, filename: &str) -> Result<()> {
    let filepath = format!("{}.mkv", filepath);
    // creates file list for ffmpeg
    let mut file_list = String::new();
    for stream in streams {
        let s = &(*stream.write().map_err(s!())?);
        if s.file.is_some() {
            let line = format!("file '{}'\n", s.filepath);
            file_list.push_str(&line);
        }
    }
    let txt_filepath = format!("{}{}.txt", util::temp_dir().map_err(s!())?, filename);
    fs::write(&txt_filepath, file_list.trim()).map_err(e!())?;
    let txt_file = FileManage::new(txt_filepath).map_err(s!())?;
    // starts ffmpeg process
    let mut child = process::Command::new(ffmpeg_path)
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(txt_file.filepath())
        .arg("-c")
        .arg("copy")
        .arg(&filepath)
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
            if fs::metadata(&filepath).is_ok() {
                fs::remove_file(filepath).map_err(e!())?;
            }
            return Err("not enough memory, killed ffmpeg").map_err(s!())?;
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
pub fn muxer(streams: &Vec<Arc<RwLock<stream::Stream>>>, filepath: &str, filename: &str, pf: platform::Platform) -> Result<()> {
    if let Some(mkvmerge_path) = mkv_exists().map_err(s!())? {
        match mkvmerge(&mkvmerge_path, streams, filepath, filename) {
            Err(e) => eprintln!("{}", e),
            _ => return Ok(()),
        }
    }
    if let Some(ffmpeg_path) = ffmpeg_exists().map_err(s!())? {
        match ffmpeg(ffmpeg_path, streams, filepath, filename) {
            Err(e) => eprintln!("{}", e),
            _ => return Ok(()),
        }
    }
    local_muxer(streams, filepath, pf).map_err(s!())?;
    Ok(())
}
fn local_muxer(streams: &Vec<Arc<RwLock<stream::Stream>>>, filepath: &str, pf: platform::Platform) -> Result<()> {
    let filepath = format!("{}.{}", filepath, platform::platform_extension(&pf));
    // creates file
    let mut file = fs::OpenOptions::new().create(true).append(true).open(filepath).map_err(e!())?;
    // muxes stream to file
    for stream in streams {
        let s = &mut (*stream.write().map_err(s!())?);
        if let Some(mut f) = s.file.take() {
            let mut data: Vec<u8> = Vec::new();
            _ = f.read_to_end(&mut data).map_err(e!())?;
            file.write_all(&data).map_err(e!())?;
            fs::remove_file(&s.filepath).map_err(e!())?;
        }
    }
    Ok(())
}
