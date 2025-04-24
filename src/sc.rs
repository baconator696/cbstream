use crate::stream::{ManagePlaylist, Playlist, Stream};
use crate::{abort, config::ModelActions, util};
use crate::{e, h, o, s};
use std::io::{Read, Write};
use std::sync::{Arc, RwLock};
use std::{thread::JoinHandle, *};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub struct ScModel {
    username: String,
    playlist_link: Option<String>,
    thread_handle: Option<JoinHandle<()>>,
    abort: Arc<RwLock<bool>>,
}
impl ScModel {
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
        // get hls url prefix
        let url = "https://stripchat.com/api/front/models?primaryTag=girls";
        let json_raw = util::get_retry(url, 5).map_err(s!())?;
        let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
        let ref_models = json["models"].as_array().ok_or_else(o!())?;
        if ref_models.len() < 1 {
            return Err("stripchat models api not as expected".into());
        }
        let ref_hls = ref_models[0]["hlsPlaylist"].as_str().ok_or_else(o!())?;
        let hls_prefix = ref_hls.split("/").collect::<Vec<&str>>()[..3].join("/");
        // get model ID
        let url = format!("https://stripchat.com/api/front/v2/models/username/{}/cam", self.username);
        let json_raw = util::get_retry(&url, 5).map_err(s!())?;
        let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
        let model_id = json["user"]["user"]["id"].as_i64().ok_or_else(o!())?;
        // get largest HLS stream
        let playlist_url = format!("{}/hls/{}/master/{}_auto.m3u8", hls_prefix, model_id, model_id);
        let playlist = match util::get_retry(&playlist_url, 5).map_err(s!()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                self.playlist_link = None;
                return Ok(());
            }
        };
        let split: Vec<&str> = playlist.split("\n").collect();
        for line in split {
            if line.len() < 5 || &line[..1] == "#" {
                continue;
            }
            self.playlist_link = Some(line.to_string());
            break;
        }
        return Ok(());
    }
}
impl ModelActions for ScModel {
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
            ScPlaylist::new(u, p, a).playlist().unwrap();
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
struct ScPlaylist {
    pl: Playlist,
    mp4_header: bool,
}
impl ScPlaylist {
    pub fn new(username: String, playlist_url: String, abort: Arc<RwLock<bool>>) -> Self {
        ScPlaylist {
            pl: Playlist::new(username, playlist_url, abort),
            mp4_header: false,
        }
    }
}
impl ManagePlaylist for ScPlaylist {
    fn playlist(&mut self) -> Result<()> {
        while !self.pl.abort_get().map_err(s!())? && !abort::get().map_err(s!())? {
            if self.pl.update_playlist().is_err() {
                break;
            }
            for stream in self.parse_playlist().map_err(s!())? {
                if let Some(last) = &self.pl.last_stream {
                    if stream <= *last.read().map_err(s!())? {
                        continue;
                    }
                }
                let stream = Arc::new(RwLock::new(stream));
                let s = stream.clone();
                let l = self.pl.last_stream.clone();
                thread::spawn(move || {
                    (*s.write().unwrap()).download(l).unwrap();
                });
                self.pl.last_stream = Some(stream);
                thread::sleep(time::Duration::from_millis(500));
            }
            thread::sleep(time::Duration::from_millis(1500));
        }
        self.mux_streams()?;
        Ok(())
    }
    fn parse_playlist(&mut self) -> Result<Vec<Stream>> {
        // determines temp directory
        let temp_dir = if cfg!(target_os = "windows") {
            let t = env::var("TEMP").map_err(e!())?;
            format!("{}\\cbstream\\", t)
        } else {
            let t = match env::var("TEMP") {
                Ok(r) => r,
                _ => format!("/tmp"),
            };
            format!("{}/cbstream/", t)
        };
        // creates temp directory
        match fs::create_dir_all(&temp_dir) {
            Err(e) => {
                if e.kind() != io::ErrorKind::AlreadyExists {
                    return Err(e).map_err(e!())?;
                }
            }
            _ => (),
        }
        let mut streams = Vec::new();
        let mut date: Option<String> = None;
        let mut mp4_header_url: Option<&str> = None;
        if let Some(playlist) = &self.pl.playlist {
            for line in playlist.lines() {
                // parses MP4 header
                if mp4_header_url.is_none() && !self.mp4_header {
                    if line.contains("EXT-X-MAP:URI") {
                        let header_url_split = line.split("\"").collect::<Vec<&str>>();
                        if header_url_split.len() < 2 {
                            return Err("can not get mp4 header file".into());
                        }
                        let header_url = header_url_split[1];
                        mp4_header_url = Some(header_url);
                    }
                }
                // parses date and time from playlist
                if date.is_none() {
                    if let Some(n) = line.find("TIME") {
                        if line.len() < 21 {
                            return Err("error parsing date from playlist")?;
                        }
                        let t = (&line[n + 7..n + 21]).replace(":", "-").replace("T", "_");
                        date = Some(t);
                    }
                }
                // adds mp4 header as initial stream
                if mp4_header_url.is_some() && !self.mp4_header {
                    let filename = match &date {
                        Some(date) => {
                            format!("SC_{}_{}", self.pl.username, date)
                        }
                        None => continue,
                    };
                    let id = 0;
                    let filepath = format!("{}sc-{}-{}.mp4h", temp_dir, self.pl.username, id);
                    streams.push(Stream::new(&filename, mp4_header_url.unwrap(), id, &filepath));
                    self.mp4_header = true;
                }
                if line.len() == 0 || &line[..1] == "#" {
                    continue;
                }
                // parses relevant information
                let url = line.to_string();
                // parses stream id
                let id = line.split("_").last().ok_or_else(o!())?;
                let n = id.find(".").ok_or_else(o!())?;
                let id = (&id[..n]).trim().parse::<u32>().map_err(e!())?;
                let filename = match &date {
                    Some(date) => {
                        format!("SC_{}_{}", self.pl.username, date)
                    }
                    None => break,
                };
                let filepath = format!("{}sc-{}-{}.mp4s", temp_dir, self.pl.username, id);
                streams.push(Stream::new(&filename, &url, id, &filepath));
            }
        }
        Ok(streams)
    }
    fn mux_streams(&mut self) -> Result<()> {
        let mut streams: Vec<sync::Arc<sync::RwLock<Stream>>> = Vec::new();
        let mut last = match self.pl.last_stream.take() {
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
        // gets os slash
        let slash = if cfg!(target_os = "windows") { "\\" } else { "/" };
        // creates output directory, places in current directory
        match fs::create_dir(&self.pl.username) {
            Err(r) => {
                if r.kind() != io::ErrorKind::AlreadyExists {
                    return Err(r).map_err(s!())?;
                }
            }
            _ => (),
        };
        // creates filename
        let filename = format!("{}{}{}.mp4", &self.pl.username, slash, streams[0].read().map_err(s!())?.filename);
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
