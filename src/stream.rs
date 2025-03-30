use crate::s;
use crate::util;
use std::sync::{Arc, RwLock};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub struct Playlist {
    pub username: String,
    url: String,
    pub playlist: Option<String>,
    pub last_stream: Option<Arc<RwLock<Stream>>>,
    pub abort: Arc<RwLock<bool>>,
    //pub muxing_handles: Vec<thread::JoinHandle<()>>,
}
impl Playlist {
    pub fn new(username: String, playlist_url: String, abort: Arc<RwLock<bool>>) -> Result<Self> {
        Ok(Playlist {
            username,
            url: playlist_url,
            playlist: None,
            last_stream: None,
            abort,
            //muxing_handles: Vec::new(),
        })
    }
    pub fn update_playlist(&mut self) -> Result<()> {
        self.playlist = Some(util::get_retry(&self.url, 5).map_err(s!())?);
        Ok(())
    }
    pub fn url_prefix(&self) -> Result<&str> {
        let mut n = 0;
        loop {
            n = match str::find(&self.url[n + 1..], "/") {
                Some(m) => m + n + 1,
                None => break,
            };
        }
        if self.url.len() < n {
            return Err("url parsing error").map_err(s!())?;
        }
        Ok(&self.url[..n])
    }
}
pub trait ManagePlaylist {
    fn playlist(&mut self) -> Result<()>;
    fn parse_playlist(&self) -> Result<Vec<Stream>>;
    fn mux_streams(&mut self) -> Result<()>;
}
pub struct Stream {
    pub name: String,
    pub url: String,
    pub time: u32,
    pub filepath: String,
    pub file: Option<fs::File>,
    pub last: Option<Arc<RwLock<Stream>>>,
}
impl Drop for Stream {
    fn drop(&mut self) {
        self.file = None;
        if self.file.is_some() {
            _ = fs::remove_file(&self.filepath)
        }
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
pub trait ManageStream {
    fn download(&mut self, last: Option<Arc<RwLock<Stream>>>) -> Result<()>;
}
