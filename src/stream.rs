use crate::{abort, e, muxer, platforms::Platform, s, util};
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
    playlist_url: String,
    pub playlist: Option<String>,
    last_stream: Option<Arc<RwLock<Stream>>>,
    abort: Arc<RwLock<bool>>,
    downloading: Arc<RwLock<bool>>,
    pub mp4_header: Option<Arc<Vec<u8>>>,
}
impl Playlist {
    pub fn new(
        platform: Platform,
        username: String,
        playlist_url: String,
        abort: Arc<RwLock<bool>>,
        downloading: Arc<RwLock<bool>>,
        mp4_header: Option<Arc<Vec<u8>>>,
    ) -> Self {
        Playlist {
            platform,
            username,
            playlist_url,
            playlist: None,
            last_stream: None,
            abort,
            downloading,
            mp4_header,
        }
    }
    /// updates downloaded playlist with url
    fn update_playlist(&mut self) -> Result<()> {
        let headers = util::create_headers(serde_json::json!({
            "user-agent": util::get_useragent().map_err(s!())?,
            "referer": self.platform.referer(),

        }))
        .map_err(s!())?;
        let playlist = util::get_retry(&self.playlist_url, 5, Some(&headers))?;
        self.playlist = Some(playlist);
        Ok(())
    }
    /// returns url prefix of playlist url
    pub fn url_prefix(&self) -> Option<&str> {
        util::url_prefix(&self.playlist_url)
    }
    fn abort_get(&self) -> Result<bool> {
        Ok(*self.abort.read().map_err(s!())?)
    }
    /// Main Playlist Loop
    pub fn playlist(&mut self) -> Result<()> {
        let mut trys = 0;
        while !self.abort_get().map_err(s!())? && !abort::get().map_err(s!())? {
            if self.update_playlist().is_err() {
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
        filepath.push(&filename);
        muxer::muxer(&streams, &filepath, self.platform.clone())?;
        Ok(())
    }
}
pub struct Stream {
    pub filename: String,
    url: String,
    id: u32,
    index: u32,
    pub stream_path: path::PathBuf,
    pub file: Option<fs::File>,
    pub last: Option<Arc<RwLock<Stream>>>,
    file_header: Option<Arc<Vec<u8>>>,
    platform: Platform,
}
impl Stream {
    pub fn new(filename: &str, url: &str, id: u32, filepath: &Path, file_header: Option<Arc<Vec<u8>>>, platform: Platform) -> Self {
        Self {
            filename: filename.to_string(),
            url: url.to_string(),
            id,
            index: 0,
            stream_path: filepath.to_path_buf(),
            file: None,
            last: None,
            file_header,
            platform,
        }
    }
}
/// downloads the stream given the Stream's url
fn download(stream: Arc<RwLock<Stream>>) -> Result<()> {
    println!("{}_{}", stream.read().map_err(s!())?.filename, stream.read().map_err(s!())?.id);
    let headers = util::create_headers(serde_json::json!({
        "user-agent": util::get_useragent().map_err(s!())?,
        "referer": stream.read().map_err(s!())?.platform.referer(),
    }))
    .map_err(s!())?;
    let data: Vec<u8> = match util::get_retry_vec(&stream.read().map_err(s!())?.url, 5, Some(&headers)).map_err(s!()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return Ok(());
        }
    };
    if data.len() < 10000 {
        return Ok(());
    }
    let mut file = fs::File::create_new(&stream.read().map_err(s!())?.stream_path).map_err(e!())?;
    if let Some(header) = &stream.read().map_err(s!())?.file_header {
        file.write_all(header).map_err(e!())?;
    }
    file.write_all(&data).map_err(e!())?;
    file.seek(io::SeekFrom::Start(0)).map_err(e!())?;
    let mut s = stream.write().map_err(s!())?;
    s.file = Some(file);
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
    }
}
impl PartialEq for Stream {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl PartialOrd for Stream {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.id.cmp(&other.id))
    }
}
