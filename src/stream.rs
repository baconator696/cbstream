use crate::{abort, muxer, platform, util};
use crate::{e, s};
use std::io::{Seek, Write};
use std::sync::{Arc, RwLock};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub struct Playlist {
    pub platform: platform::Platform,
    pub username: String,
    playlist_url: String,
    pub playlist: Option<String>,
    last_stream: Option<Arc<RwLock<Stream>>>,
    abort: Arc<RwLock<bool>>,
    downloading: Arc<RwLock<bool>>,
    pub mp4_header: Option<Arc<Vec<u8>>>,
}
impl Playlist {
    /// creates Playlist struct
    pub fn new(
        platform: platform::Platform,
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
        self.playlist = Some(util::get_retry(&self.playlist_url, 5)?);
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
        while !self.abort_get().map_err(s!())? && !abort::get().map_err(s!())? {
            if self.update_playlist().is_err() {
                break;
            }
            for stream in self.parse_playlist() {
                if let Some(last) = &self.last_stream {
                    if stream <= *last.read().map_err(s!())? {
                        continue;
                    }
                }
                let stream = Arc::new(RwLock::new(stream));
                let s = stream.clone();
                let l = self.last_stream.clone();
                thread::spawn(move || {
                    (*s.write().unwrap()).download(l).unwrap();
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
        match platform::parse_playlist(self) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                Vec::new()
            }
        }
    }
    fn mux_streams(&mut self) -> Result<()> {
        let mut streams: Vec<sync::Arc<sync::RwLock<Stream>>> = Vec::new();
        let mut last = match self.last_stream.take() {
            Some(o) => o,
            None => return Ok(()),
        };
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
        util::create_dir(&self.username).map_err(s!())?;
        // creates filename
        let filename = streams[0].read().map_err(s!())?.filename.clone();
        let filepath = format!("{}{}{}", &self.username, util::SLASH, filename);
        muxer::muxer(&streams, &filepath, &filename, self.platform.clone())?;
        Ok(())
    }
}
pub struct Stream {
    pub filename: String,
    url: String,
    id: u32,
    pub filepath: String,
    pub file: Option<fs::File>,
    pub last: Option<Arc<RwLock<Stream>>>,
    file_header: Option<Arc<Vec<u8>>>,
}
impl Stream {
    pub fn new(filename: &str, url: &str, id: u32, filepath: &str, file_header: Option<Arc<Vec<u8>>>) -> Self {
        Self {
            filename: filename.to_string(),
            url: url.to_string(),
            id,
            filepath: filepath.to_string(),
            file: None,
            last: None,
            file_header,
        }
    }
    /// downloads the stream given the Stream's url
    pub fn download(&mut self, last: Option<Arc<RwLock<Stream>>>) -> Result<()> {
        println!("{}_{}", self.filename, self.id);
        self.last = last;
        let data: Vec<u8> = match util::get_retry_vec(&self.url, 5).map_err(s!()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                return Ok(());
            }
        };
        if data.len() < 10000 {
            return Ok(());
        }
        let mut file = fs::File::create_new(&self.filepath).map_err(e!())?;
        if let Some(header) = &self.file_header {
            file.write_all(header).map_err(e!())?;
        }
        file.write_all(&data).map_err(e!())?;
        file.seek(io::SeekFrom::Start(0)).map_err(e!())?;
        self.file = Some(file);
        Ok(())
    }
}
impl Drop for Stream {
    /// removes downloaded stream file
    fn drop(&mut self) {
        if self.file.is_some() {
            _ = fs::remove_file(&self.filepath)
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
