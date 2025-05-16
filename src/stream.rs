use crate::util;
use crate::{e, s};
use std::io::{Read, Seek, Write};
use std::sync::{Arc, RwLock};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub trait ManagePlaylist {
    /// main download loop for the playlist
    fn playlist(&mut self) -> Result<()>;
    /// parses playlist into given streams
    fn parse_playlist(&mut self) -> Result<Vec<Stream>>;
    /// muxes all downloaded streams when stream finishes or is canceled
    fn mux_streams(&mut self) -> Result<()>;
}
pub struct Playlist {
    pub username: String,
    playlist_url: String,
    pub playlist: Option<String>,
    pub last_stream: Option<Arc<RwLock<Stream>>>,
    abort: Arc<RwLock<bool>>,
    pub mp4_header: Option<Arc<Vec<u8>>>,
    //pub muxing_handles: Vec<thread::JoinHandle<()>>,
}
impl Playlist {
    /// creates Playlist struct
    pub fn new(username: String, playlist_url: String, abort: Arc<RwLock<bool>>, mp4_header: Option<Arc<Vec<u8>>>) -> Self {
        Playlist {
            username,
            playlist_url,
            playlist: None,
            last_stream: None,
            abort,
            mp4_header,
            //muxing_handles: Vec::new(),
        }
    }
    /// updates download playlist with url
    pub fn update_playlist(&mut self) -> Result<()> {
        self.playlist = Some(util::get_retry(&self.playlist_url, 5).map_err(s!())?);
        Ok(())
    }
    /// returns url prefix of playlist url
    pub fn url_prefix(&self) -> Option<&str> {
        util::url_prefix(&self.playlist_url)
    }
    pub fn abort_get(&self) -> Result<bool> {
        Ok(*self.abort.read().map_err(s!())?)
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
pub fn mux_streams(last_stream: &mut Option<Arc<RwLock<Stream>>>, username: &str, file_extension: &str) -> Result<()> {
    let mut streams: Vec<sync::Arc<sync::RwLock<Stream>>> = Vec::new();
    let mut last = match last_stream.take() {
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
    util::create_dir(username).map_err(s!())?;
    // creates filename
    let filename = format!(
        "{}{}{}.{}",
        username,
        util::SLASH,
        streams[0].read().map_err(s!())?.filename,
        file_extension
    );
    // creates file
    let mut file = fs::OpenOptions::new().create(true).append(true).open(filename).map_err(e!())?;
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
