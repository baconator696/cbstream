use {
    crate::{
        e, h, o,
        platforms::Platform,
        s,
        util::{self, ManagedFile},
    },
    std::{io::Read, process::ExitStatus, *},
};
type Res<T> = Result<T, Box<dyn error::Error>>;
type Hres<T> = Result<T, String>;
fn ffmpeg_exists() -> Res<Option<&'static str>> {
    let path = "ffmpeg";
    match process::Command::new(path).arg("-version").output() {
        Ok(_) => Ok(Some(path)),
        Err(_) => Ok(None),
    }
}
/// muxes streams with ffmpeg pipe
fn ffmpeg_seperate_v_a(
    ffmpeg_path: &str,
    file: &ManagedFile,
    file_audio: &Option<ManagedFile>,
    pf: &Platform,
) -> Res<()> {
    let mut filepath = file.final_path.clone();
    filepath.set_extension("mkv");
    let mut container_type = match pf {
        Platform::CB => "mpegts",
        Platform::MFC => "mpegts",
        Platform::SC => "mp4",
        Platform::SCVR => "mp4",
        Platform::BONGA => "mpegts",
        Platform::SODA => "mp4",
    };
    if file_audio.is_some() {
        container_type = "mp4";
    }
    // starts ffmpeg process
    let mut command = process::Command::new(ffmpeg_path);
    command
        .arg("-f")
        .arg(container_type)
        .arg("-i")
        .arg(&file.path);
    if let Some(file_audio) = file_audio {
        command
            .arg("-f")
            .arg(container_type)
            .arg("-i")
            .arg(&file_audio.path);
    }
    let mut child = command
        .arg("-c")
        .arg("copy")
        .arg("-copyts")
        .arg("-avoid_negative_ts")
        .arg("make_zero")
        .arg("-y")
        .arg(&filepath)
        .stderr(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .spawn()
        .map_err(e!())?;
    // read from stderr/stdout pipes
    let mut stdout = child.stdout.take().ok_or_else(o!())?;
    let mut stderr = child.stderr.take().ok_or_else(o!())?;
    let stdout_handle = thread::spawn(move || -> Hres<String> {
        let mut out = String::new();
        stdout.read_to_string(&mut out).map_err(e!())?;
        Ok(out)
    });
    let stderr_handle = thread::spawn(move || -> Hres<String> {
        let mut out = String::new();
        stderr.read_to_string(&mut out).map_err(e!())?;
        Ok(out)
    });
    // monitors system memory
    let kill_handle = thread::spawn(move || -> Hres<ExitStatus> {
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
    // cleanup
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
pub fn muxer(file: ManagedFile, file_audio: Option<ManagedFile>, pf: Platform) -> Res<()> {
    util::create_dir(file.final_path.parent().ok_or_else(o!())?).map_err(s!())?;
    if let Some(ffmpeg_path) = ffmpeg_exists().map_err(s!())? {
        match ffmpeg_seperate_v_a(ffmpeg_path, &file, &file_audio, &pf) {
            Err(e) => eprintln!("{}", e),
            Ok(_) => return Ok(()),
        }
    }
    local_muxer(file, file_audio, pf).map_err(s!())?;
    Ok(())
}
/// Fallback local muxer
fn local_muxer(file: ManagedFile, file_audio: Option<ManagedFile>, pf: Platform) -> Res<()> {
    let mut extension = match pf {
        Platform::CB => "ts",
        Platform::MFC => "ts",
        Platform::SC => "mp4",
        Platform::SCVR => "mp4",
        Platform::BONGA => "ts",
        Platform::SODA => "mp4",
    };
    if file_audio.is_some() {
        extension = "mp4";
    }
    let mut filepath = file.final_path.clone();
    filepath.set_extension(extension);
    file.mv(&filepath).map_err(s!())?;
    if let Some(file_audio) = file_audio.as_ref() {
        let mut filepath_audio = file_audio.final_path.clone();
        filepath_audio.set_extension("m4a");
        file_audio.mv(&filepath_audio).map_err(s!())?;
    }
    Ok(())
}
