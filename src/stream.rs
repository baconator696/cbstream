use crate::util;
use crate::{e, s};
use std::io::{Seek, Write};
use std::sync::{Arc, RwLock};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub trait ManagePlaylist {
    /// main download loop for the playlist
    fn playlist(&mut self) -> Result<()>;
    /// parses chaturbate playlist into given streams
    fn parse_playlist(&self) -> Result<Vec<Stream>>;
    /// muxes all downloaded streams when stream finishes or is canceled
    fn mux_streams(&mut self) -> Result<()>;
}
pub struct Playlist {
    pub username: String,
    playlist_url: String,
    pub playlist: Option<String>,
    pub last_stream: Option<Arc<RwLock<Stream>>>,
    abort: Arc<RwLock<bool>>,
    //pub muxing_handles: Vec<thread::JoinHandle<()>>,
}
impl Playlist {
    /// creates Playlist struct
    pub fn new(username: String, playlist_url: String, abort: Arc<RwLock<bool>>) -> Self {
        Playlist {
            username,
            playlist_url,
            playlist: None,
            last_stream: None,
            abort,
            //muxing_handles: Vec::new(),
        }
    }
    /// updates download playlist with url
    pub fn update_playlist(&mut self) -> Result<()> {
        self.playlist = Some(util::get_retry(&self.playlist_url, 5).map_err(s!())?);
        Ok(())
    }
    /// returns url prefix of playlist url
    pub fn url_prefix(&self) -> Result<&str> {
        let mut n = 0;
        loop {
            n = match str::find(&self.playlist_url[n + 1..], "/") {
                Some(m) => m + n + 1,
                None => break,
            };
        }
        if self.playlist_url.len() < n {
            return Err("url parsing error").map_err(s!())?;
        }
        Ok(&self.playlist_url[..n])
    }
    pub fn abort_get(&self) -> Result<bool> {
        Ok(*self.abort.read().map_err(s!())?)
    }
}
pub struct Stream {
    pub filename: String,
    pub url: String,
    pub time: u32,
    pub filepath: String,
    pub file: Option<fs::File>,
    pub last: Option<Arc<RwLock<Stream>>>,
}
impl Stream {
    /// downloads the stream given the Stream's url
    pub fn download(&mut self, last: Option<Arc<RwLock<Stream>>>) -> Result<()> {
        println!("{}_{}", self.filename, self.time);
        self.last = last;
        let data = util::get_retry_vec(&self.url, 5).map_err(s!())?;
        let mut file = fs::File::create_new(&self.filepath).map_err(e!())?;
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
        self.file = None;
    }
}
impl PartialEq for Stream {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}
impl PartialOrd for Stream {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.time.cmp(&other.time))
    }
}
