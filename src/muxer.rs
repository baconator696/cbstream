use crate::platforms::Platform;
use crate::stream::Stream;
use crate::util;
use crate::{e, h, o, s};
use std::io::{Read, Write};
use std::process::ExitStatus;
use std::sync::{Arc, RwLock};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
type Hresult<T> = result::Result<T, String>;
/// file management
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
/// checks if mkvtoolnix is installed and returns path of mkvmerger
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
/// muxes streams with mkvmerge
fn mkvmerge(mkvmerge_path: &str, streams: &Vec<Arc<RwLock<Stream>>>, filepath: &str, filename: &str) -> Result<()> {
    let filepath = format!("{}.mkv", filepath);
    // creates arg list for mkvmerge
    let mut arg_list: Vec<String> = Vec::with_capacity(streams.len() * 2 + 2);
    arg_list.push("-o".into());
    arg_list.push(filepath.clone());
    for stream in streams {
        let s = &(*stream.read().map_err(s!())?);
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
/// muxes streams with ffmpeg pipe
fn ffmpeg(ffmpeg_path: &str, streams: &Vec<Arc<RwLock<Stream>>>, filepath: &str, pf: &Platform) -> Result<()> {
    let filepath = format!("{}.mkv", filepath);
    let container_type = match pf {
        Platform::CB => "mpegts",
        Platform::MFC => "mpegts",
        Platform::SC => "mp4",
        Platform::SCVR => "mp4",
        Platform::BONGA => "mpegts",
    };
    // starts ffmpeg process
    let mut child = process::Command::new(ffmpeg_path)
        .arg("-f")
        .arg(container_type)
        .arg("-i")
        .arg("pipe:0")
        .arg("-c")
        .arg("copy")
        .arg("-y")
        .arg(&filepath)
        .stderr(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stdin(process::Stdio::piped())
        .spawn()
        .map_err(e!())?;
    // read from stderr/stdout pipes
    let mut stdout = child.stdout.take().ok_or_else(o!())?;
    let mut stderr = child.stderr.take().ok_or_else(o!())?;
    let mut stdin = child.stdin.take().ok_or_else(o!())?;
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
    let kill_handle = thread::spawn(move || -> Hresult<ExitStatus> {
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
        Ok(exit_status)
    });
    // pipe data into ffmpeg
    let mut buffer = vec![0u8; 1 << 16];
    'a: for stream in streams {
        let s = &(*stream.read().map_err(s!())?);
        if let Some(f) = s.file.as_ref() {
            let mut reader = io::BufReader::new(f);
            loop {
                let n = reader.read(&mut buffer).map_err(e!())?;
                if n == 0 {
                    break;
                }
                match stdin.write_all(&buffer[..n]).map_err(e!()) {
                    Ok(_) => (),
                    Err(e) => {
                        eprintln!("{}", e);
                        break 'a;
                    }
                };
            }
        }
    }
    drop(stdin);
    let exit_status = kill_handle.join().map_err(h!())?.map_err(s!())?;
    let stdout = stdout_handle.join().map_err(h!())?.map_err(s!())?;
    let stderr = stderr_handle.join().map_err(h!())?.map_err(s!())?;
    // processes output
    if !exit_status.success() {
        return Err(format!("{}{}", stdout.trim(), stderr.trim())).map_err(s!())?;
    }
    return Ok(());
}
/// Main Muxing Function
pub fn muxer(streams: &Vec<Arc<RwLock<Stream>>>, filepath: &str, filename: &str, pf: Platform) -> Result<()> {
    if let Some(ffmpeg_path) = ffmpeg_exists().map_err(s!())? {
        match ffmpeg(ffmpeg_path, streams, filepath, &pf) {
            Err(e) => eprintln!("{}", e),
            Ok(_) => return Ok(()),
        }
    }
    if let Some(mkvmerge_path) = mkv_exists().map_err(s!())? {
        match mkvmerge(&mkvmerge_path, streams, filepath, filename) {
            Err(e) => eprintln!("{}", e),
            Ok(_) => return Ok(()),
        }
    }
    local_muxer(streams, filepath, pf).map_err(s!())?;
    Ok(())
}
/// Fallback local muxer
fn local_muxer(streams: &Vec<Arc<RwLock<Stream>>>, filepath: &str, pf: Platform) -> Result<()> {
    let extension = match pf {
        Platform::CB => "ts",
        Platform::MFC => "ts",
        Platform::SC => "mp4",
        Platform::SCVR => "mp4",
        Platform::BONGA => "ts",
    };
    let filepath = format!("{}.{}", filepath, extension);
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
