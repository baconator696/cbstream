use crate::stream::{ManagePlaylist, Playlist, Stream};
use crate::{abort, config::ModelActions, util};
use crate::{e, h, o, s};
use std::io::{Read, Write};
use std::sync::{Arc, RwLock};
use std::{thread::JoinHandle, *};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub struct CbModel {
    username: String,
    playlist_link: Option<String>,
    thread_handle: Option<JoinHandle<()>>,
    abort: Arc<RwLock<bool>>,
}
impl CbModel {
    /// creates Cb struct
    pub fn new(username: &str) -> Self {
        Self {
            username: username.to_string(),
            playlist_link: None,
            thread_handle: None,
            abort: Arc::new(RwLock::new(false)),
        }
    }
    /// downloads the latest playlist
    fn get_playlist(&mut self) -> Result<()> {
        // get model playlist link
        let url = format!("https://chaturbate.com/api/chatvideocontext/{}/", self.username);
        let json_raw = match util::get_retry(&url, 1).map_err(s!()) {
            Ok(r) => r,
            Err(e) => {
                if !e.contains("Unauthorized") {
                    eprintln!("{}", e);
                }
                self.playlist_link = None;
                return Ok(());
            }
        };
        let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
        let playlist_url = json["hls_source"].as_str().ok_or_else(o!())?;
        if playlist_url.len() == 0 {
            self.playlist_link = None;
            return Ok(());
        }
        // get playlist of resolutions
        let playlist = match util::get_retry(&playlist_url, 5).map_err(s!()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                self.playlist_link = None;
                return Ok(());
            }
        };
        let mut split = playlist.lines().collect::<Vec<&str>>();
        split.reverse();
        for line in split {
            if line.len() < 5 || &line[..1] == "#" {
                continue;
            }
            self.playlist_link = Some(format!("{}/{}", util::url_prefix(playlist_url).ok_or_else(o!())?, line));
            break;
        }
        return Ok(());
    }
}
impl ModelActions for CbModel {
    fn is_online(&mut self) -> Result<bool> {
        self.get_playlist().map_err(s!())?;
        Ok(self.playlist_link.is_some())
    }
    fn is_finished(&self) -> bool {
        if let Some(h) = &self.thread_handle { h.is_finished() } else { true }
    }
    fn clean_handle(&mut self) -> Result<()> {
        if let Some(h) = self.thread_handle.take() {
            Ok(h.join().map_err(h!())?)
        } else {
            Ok(())
        }
    }
    fn download(&mut self) -> Result<()> {
        let u = self.username.clone();
        let a = self.abort.clone();
        let p = self.playlist_link.clone().ok_or_else(o!())?;
        let handle: thread::JoinHandle<()> = thread::spawn(move || {
            CbPlaylist::new(u, p, a).playlist().unwrap();
        });
        if let Some(h) = self.thread_handle.replace(handle) {
            h.join().map_err(h!())?;
        }
        Ok(())
    }
    fn abort(&self) -> Result<()> {
        *self.abort.write().map_err(s!())? = true;
        Ok(())
    }
}
struct CbPlaylist(Playlist);
impl CbPlaylist {
    pub fn new(username: String, playlist_url: String, abort: Arc<RwLock<bool>>) -> Self {
        CbPlaylist(Playlist::new(username, playlist_url, abort))
    }
}
impl ManagePlaylist for CbPlaylist {
    fn playlist(&mut self) -> Result<()> {
        while !self.0.abort_get().map_err(s!())? && !abort::get().map_err(s!())? {
            if self.0.update_playlist().is_err() {
                break;
            }
            for stream in self.parse_playlist().map_err(s!())? {
                if let Some(last) = &self.0.last_stream {
                    if stream <= *last.read().map_err(s!())? {
                        continue;
                    }
                }
                let stream = Arc::new(RwLock::new(stream));
                let s = stream.clone();
                let l = self.0.last_stream.clone();
                thread::spawn(move || {
                    (*s.write().unwrap()).download(l).unwrap();
                });
                self.0.last_stream = Some(stream);
                thread::sleep(time::Duration::from_millis(500));
            }
            thread::sleep(time::Duration::from_millis(1500));
        }
        self.mux_streams()?;
        Ok(())
    }
    fn parse_playlist(&mut self) -> Result<Vec<Stream>> {
        let temp_dir = util::temp_dir().map_err(s!())?;
        util::create_dir(&temp_dir).map_err(s!())?;
        let mut streams = Vec::new();
        let mut date: Option<String> = None;
        if let Some(playlist) = &self.0.playlist {
            for line in playlist.split("\n") {
                // parses date and time from playlist
                if let Some(n) = line.find("TIME") {
                    if line.len() < 21 {
                        return Err("error parsing date from playlist")?;
                    }
                    let t = (&line[n + 7..n + 21]).replace(":", "-").replace("T", "_");
                    date = Some(t);
                }
                if line.len() == 0 || &line[..1] == "#" {
                    continue;
                }
                let full_url = format!("{}/{}", self.0.url_prefix().ok_or_else(o!())?, line);
                // parses stream id
                let id = line.split("_").last().ok_or_else(o!())?;
                let n = id.find(".").ok_or_else(o!())?;
                let id = (&id[..n]).trim().parse::<u32>().map_err(e!())?;
                let filename = match &date {
                    Some(date) => {
                        format!("CB_{}_{}", self.0.username, date)
                    }
                    None => break,
                };
                let filepath = format!("{}cb-{}-{}.ts", temp_dir, self.0.username, id);
                streams.push(Stream::new(&filename, &full_url, id, &filepath));
            }
        }
        Ok(streams)
    }
    fn mux_streams(&mut self) -> Result<()> {
        let mut streams: Vec<sync::Arc<sync::RwLock<Stream>>> = Vec::new();
        let mut last = match self.0.last_stream.take() {
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
        util::create_dir(&self.0.username).map_err(s!())?;
        // creates filename
        let filename = format!("{}{}{}.ts", &self.0.username, util::SLASH, streams[0].read().map_err(s!())?.filename);
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
}
