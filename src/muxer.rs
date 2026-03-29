use crate::{e, h, o, platforms::Platform, s, stream::Stream};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::ExitStatus,
    sync::{Arc, RwLock},
    *,
};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
type Hresult<T> = result::Result<T, String>;
fn ffmpeg_exists() -> Result<Option<&'static str>> {
    let path = "ffmpeg";
    match process::Command::new(path).arg("-version").output() {
        Ok(_) => Ok(Some(path)),
        Err(_) => Ok(None),
    }
}
/// muxes streams with ffmpeg pipe
fn ffmpeg(ffmpeg_path: &str, streams: &Vec<Arc<RwLock<Stream>>>, filepath: &Path, pf: &Platform) -> Result<()> {
    let mut filepath = filepath.to_path_buf();
    filepath.set_extension("mkv");
    let container_type = match pf {
        Platform::CB => "mpegts",
        Platform::MFC => "mpegts",
        Platform::SC => "mp4",
        Platform::SCVR => "mp4",
        Platform::BONGA => "mpegts",
        Platform::SODA => "mp4",
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
pub fn muxer(streams: &Vec<Arc<RwLock<Stream>>>, filepath: &Path, filepath_audio: Option<PathBuf>, pf: Platform) -> Result<()> {
    if let Some(ffmpeg_path) = ffmpeg_exists().map_err(s!())? {
        match ffmpeg(ffmpeg_path, streams, filepath, &pf) {
            Err(e) => eprintln!("{}", e),
            Ok(_) => return Ok(()),
        }
    }
    local_muxer(streams, filepath, filepath_audio, pf).map_err(s!())?;
    Ok(())
}
/// Fallback local muxer
fn local_muxer(streams: &Vec<Arc<RwLock<Stream>>>, filepath: &Path, filepath_audio: Option<PathBuf>, pf: Platform) -> Result<()> {
    let mut extension = match pf {
        Platform::CB => "ts",
        Platform::MFC => "ts",
        Platform::SC => "mp4",
        Platform::SCVR => "mp4",
        Platform::BONGA => "ts",
        Platform::SODA => "mp4",
    };
    if filepath_audio.is_some() {
        extension = "mp4";
    }
    let mut filepath = filepath.to_path_buf();
    filepath.set_extension(extension);
    // creates file
    let mut file = fs::OpenOptions::new().create(true).append(true).open(filepath).map_err(e!())?;
    let mut file_audio = if let Some(filepath_audio) = filepath_audio {
        let mut filepath_audio = filepath_audio.to_path_buf();
        filepath_audio.set_extension(extension);
        Some(fs::OpenOptions::new().create(true).append(true).open(filepath_audio).map_err(e!())?)
    } else {
        None
    };
    // muxes stream to file
    for stream in streams {
        let mut buffer = vec![0u8; 1 << 16];
        let s = &mut (*stream.write().map_err(s!())?);
        if let Some(mut f) = s.file.take() {
            loop {
                let n = f.read(&mut buffer).map_err(e!())?;
                if n == 0 {
                    break;
                }
                file.write_all(&buffer[..n]).map_err(e!())?;
            }
            fs::remove_file(&s.stream_path).map_err(e!())?;
        }
        if file_audio.is_some() {
            let mut buffer = vec![0u8; 1 << 16];
            if let Some(mut f) = s.file_audio.take() {
                loop {
                    let n = f.read(&mut buffer).map_err(e!())?;
                    if n == 0 {
                        break;
                    }
                    file_audio.as_mut().unwrap().write_all(&buffer[..n]).map_err(e!())?;
                }
                fs::remove_file(&s.stream_path_audio).map_err(e!())?;
            }
        }
    }
    Ok(())
}
