use crate::{abort, debug_eprintln, e, muxer, platforms::Platform, s, util};
use std::{
    io::{Seek, Write},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    *,
};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub struct Playlist {
    pub platform: Platform,
    pub username: String,
    pub playlist_url: String,
    pub playlist_audio_url: Option<String>,
    pub playlist: Option<String>,
    pub playlist_audio: Option<String>,
    last_stream: Option<Arc<RwLock<Stream>>>,
    abort: Arc<RwLock<bool>>,
    downloading: Arc<RwLock<bool>>,
    /// video header
    pub mp4_header: Option<Arc<Vec<u8>>>,
    /// optional - for audio/video split streams
    pub mp4_header_audio: Option<Arc<Vec<u8>>>,
}
impl Playlist {
    pub fn new(
        platform: Platform,
        username: String,
        playlist_url: String,
        playlist_audio_url: Option<String>,
        abort: Arc<RwLock<bool>>,
        downloading: Arc<RwLock<bool>>,
        mp4_header: Option<Arc<Vec<u8>>>,
        mp4_header_audio: Option<Arc<Vec<u8>>>,
    ) -> Self {
        Playlist {
            platform,
            username,
            playlist_url,
            playlist_audio_url,
            playlist: None,
            playlist_audio: None,
            last_stream: None,
            abort,
            downloading,
            mp4_header,
            mp4_header_audio: mp4_header_audio,
        }
    }
    /// updates downloaded playlist with url
    fn update_playlist(&mut self) -> Result<()> {
        let headers = util::create_headers(serde_json::json!({
            "user-agent": util::get_useragent().map_err(s!())?,
            "referer": self.platform.referer(),

        }))
        .map_err(s!())?;
        let playlist = util::get_retry(&self.playlist_url, 5, Some(&headers)).map_err(s!())?;
        self.playlist = Some(playlist);
        if let Some(playlist_audio_url) = &self.playlist_audio_url {
            let playlist_audio = util::get_retry(playlist_audio_url, 5, Some(&headers)).map_err(s!())?;
            self.playlist_audio = Some(playlist_audio);
        }
        Ok(())
    }
    fn abort_get(&self) -> Result<bool> {
        Ok(*self.abort.read().map_err(s!())?)
    }
    /// Main Playlist Loop
    pub fn playlist(&mut self) -> Result<()> {
        let mut trys = 0;
        while !self.abort_get().map_err(s!())? && !abort::get().map_err(s!())? {
            let state = self.update_playlist();
            if state.is_err() {
                debug_eprintln!("{:?}", state.map_err(s!()).unwrap_err());
                break;
            }
            trys += 1;
            if trys > 10 {
                break;
            }
            for mut stream in self.parse_playlist() {
                if let Some(last) = &self.last_stream {
                    if stream <= *last.read().map_err(s!())? {
                        continue;
                    }
                }
                trys = 0;
                stream.last = self.last_stream.clone();
                stream.index = match stream.last.as_ref() {
                    Some(o) => o.read().map_err(s!())?.index + 1,
                    None => 1,
                };
                let stream = Arc::new(RwLock::new(stream));
                let s: Arc<RwLock<Stream>> = stream.clone();
                thread::spawn(move || {
                    download(s).unwrap();
                });
                self.last_stream = Some(stream);
                thread::sleep(time::Duration::from_millis(500));
            }
            thread::sleep(time::Duration::from_millis(1500));
        }
        *self.downloading.write().map_err(s!())? = false;
        self.mux_streams()?;
        Ok(())
    }
    fn parse_playlist(&mut self) -> Vec<Stream> {
        match self.platform.parse_playlist()(self) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                Vec::new()
            }
        }
    }
    fn mux_streams(&mut self) -> Result<()> {
        let mut last = match self.last_stream.take() {
            Some(o) => o,
            None => return Ok(()),
        };
        let size = last.read().map_err(s!())?.index as usize;
        let mut streams = Vec::with_capacity(size);
        // adds all streams to an iterator
        loop {
            streams.push(last.clone());
            let l = match last.write().map_err(s!())?.last.take() {
                Some(o) => o,
                None => break,
            };
            last = l;
        }
        if streams.len() == 0 {
            return Ok(());
        }
        streams.reverse();
        util::create_dir(Path::new(&self.username)).map_err(s!())?;
        // creates filename
        let filename = streams[0].read().map_err(s!())?.filename.clone();
        let mut filepath = PathBuf::from(&self.username);
        let mut filepath_audio = filepath.clone();
        filepath.push(&filename);
        let filepath_audio_option = if streams[0].read().map_err(s!())?.url_audio.is_some() {
            filepath_audio.push(format!("{}_audio", filename));
            Some(filepath_audio)
        } else {
            None
        };
        let streams = Arc::new(RwLock::new(streams));
        muxer::muxer(streams, &filepath, filepath_audio_option, self.platform.clone())?;
        Ok(())
    }
}
pub struct Stream {
    pub filename: String,
    url: String,
    url_audio: Option<String>,
    stream_id: u32,
    index: u32,
    pub stream_path: path::PathBuf,
    /// audio path generated when downloaded
    pub stream_path_audio: path::PathBuf,
    pub file: Option<fs::File>,
    pub file_audio: Option<fs::File>,
    pub last: Option<Arc<RwLock<Stream>>>,
    file_header: Option<Arc<Vec<u8>>>,
    file_header_audio: Option<Arc<Vec<u8>>>,
    platform: Platform,
}
impl Stream {
    pub fn new(
        filename: &str,
        url: &str,
        url_audio: Option<&str>,
        id: u32,
        filepath: &Path,
        file_header: Option<Arc<Vec<u8>>>,
        file_header_audio: Option<Arc<Vec<u8>>>,
        platform: Platform,
    ) -> Self {
        let stream_path_audio = match filepath.file_name() {
            Some(path_filename) => filepath.with_file_name(format!("audio_{}", path_filename.to_string_lossy())),
            None => filepath.with_file_name(format!("audio_{}", filename)),
        };
        let url_audio = if url_audio.is_none() {
            None
        } else {
            Some(url_audio.unwrap().to_string())
        };
        Self {
            filename: filename.to_string(),
            url: url.to_string(),
            url_audio: url_audio,
            stream_id: id,
            index: 0,
            stream_path: filepath.to_path_buf(),
            stream_path_audio,
            file: None,
            file_audio: None,
            last: None,
            file_header,
            file_header_audio,
            platform,
        }
    }
}
/// downloads the stream given the Stream's url
fn download(stream_lock: Arc<RwLock<Stream>>) -> Result<()> {
    // quick read to not lock stream
    let stream = stream_lock.read().map_err(s!())?;
    let filename = stream.filename.clone();
    let stream_id = stream.filename.clone();
    let referer = stream.platform.referer();
    let url_video = stream.url.clone();
    let url_audio = stream.url_audio.clone();
    drop(stream);
    println!("{}_{}", filename, stream_id);
    let headers = util::create_headers(serde_json::json!({
        "user-agent": util::get_useragent().map_err(s!())?,
        "referer": referer,
    }))
    .map_err(s!())?;
    let video_data: Vec<u8> = match util::get_retry_vec(&url_video, 5, Some(&headers)).map_err(s!()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}:{}", e, &url_video);
            return Ok(());
        }
    };
    if video_data.len() < 10000 {
        debug_eprintln!("{}", String::from_utf8_lossy(&video_data));
        return Ok(());
    }
    let mut audio_data: Option<Vec<u8>> = None;
    // for seperate audio track
    if let Some(url_audio) = &url_audio {
        let audio_data_internal: Vec<u8> = match util::get_retry_vec(url_audio, 5, Some(&headers)).map_err(s!()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}:{}", e, url_audio);
                Vec::new()
            }
        };
        if audio_data_internal.len() != 0 {
            audio_data = Some(audio_data_internal);
        };
    };
    // early write lock to guarantee file deletion
    let stream = &mut *stream_lock.write().map_err(s!())?;
    let file = fs::File::create_new(&stream.stream_path).map_err(e!())?;
    stream.file = Some(file);
    let file = stream.file.as_mut().unwrap();
    if let Some(header) = stream.file_header.as_ref() {
        file.write_all(&header).map_err(e!())?;
    }
    file.write_all(&video_data).map_err(e!())?;
    file.seek(io::SeekFrom::Start(0)).map_err(e!())?;
    // for seperate audio track
    if let Some(data_audio) = audio_data {
        let file_audio = fs::File::create_new(&stream.stream_path_audio).map_err(e!())?;
        stream.file_audio = Some(file_audio);
        let file_audio = stream.file_audio.as_mut().unwrap();
        if let Some(header_audio) = &stream.file_header_audio {
            file_audio.write_all(header_audio).map_err(e!())?;
        }
        file_audio.write_all(&data_audio).map_err(e!())?;
        file_audio.seek(io::SeekFrom::Start(0)).map_err(e!())?;
    }
    Ok(())
}
impl Drop for Stream {
    /// removes downloaded stream file
    fn drop(&mut self) {
        if self.file.is_some() {
            match fs::remove_file(&self.stream_path).map_err(e!()) {
                Err(e) => eprintln!("{}", e),
                _ => (),
            };
        }
        if self.file_audio.is_some() {
            match fs::remove_file(&self.stream_path_audio).map_err(e!()) {
                Err(e) => eprintln!("{}", e),
                _ => (),
            };
        }
    }
}
impl PartialEq for Stream {
    fn eq(&self, other: &Self) -> bool {
        self.stream_id == other.stream_id
    }
}
impl PartialOrd for Stream {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.stream_id.cmp(&other.stream_id))
    }
}
