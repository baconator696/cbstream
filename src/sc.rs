use crate::stream::{self, ManagePlaylist, Playlist, Stream};
use crate::{abort, config::ModelActions, util};
use crate::{e, h, o, s};
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
        self.playlist_link = get_playlist(&self.username, false).map_err(s!())?;
        Ok(())
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
struct ScPlaylist(Playlist);
impl ScPlaylist {
    pub fn new(username: String, playlist_url: String, abort: Arc<RwLock<bool>>) -> Self {
        ScPlaylist(Playlist::new(username, playlist_url, abort, None))
    }
}
impl ManagePlaylist for ScPlaylist {
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
        parse_playlist(&self.0.playlist, &mut self.0.mp4_header, &self.0.username, false)
    }
    fn mux_streams(&mut self) -> Result<()> {
        stream::mux_streams(&mut self.0.last_stream, &self.0.username, "mp4")
    }
}
pub fn get_playlist(username: &str, vr: bool) -> Result<Option<String>> {
    // get hls url prefix
    let url = "https://stripchat.com/api/front/models?primaryTag=girls";
    let json_raw = match util::get_retry(url, 5).map_err(s!()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return Ok(None);
        }
    };
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let ref_hls = json["models"].as_array().ok_or_else(o!())?.get(0).ok_or_else(o!())?["hlsPlaylist"]
        .as_str()
        .ok_or_else(o!())?;
    let hls_prefix = ref_hls.split("/").collect::<Vec<&str>>().get(..3).ok_or_else(o!())?.join("/");
    // get model ID
    let url = format!("https://stripchat.com/api/front/v2/models/username/{}/cam", username);
    let json_raw = match util::get_retry(&url, 5).map_err(s!()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return Ok(None);
        }
    };
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let model_id = json["user"]["user"]["id"].as_i64().ok_or_else(o!())?;
    // get largest HLS stream
    let vr = if vr { "_vr" } else { "" };
    let playlist_url = format!("{}/hls/{}{}/master/{}{}.m3u8", hls_prefix, model_id, vr, model_id, vr);
    // below is the transoded streams, (maybe add resolution settings in future)
    //let playlist_url = format!("{}/hls/{}_vr/master/{}_vr_auto.m3u8", hls_prefix, model_id, model_id);
    let playlist = match util::get_retry(&playlist_url, 1).map_err(s!()) {
        Ok(r) => r,
        _ => {
            return Ok(None);
        }
    };
    let split = playlist.lines().collect::<Vec<&str>>();
    for line in split {
        if line.len() < 5 || &line[..1] == "#" {
            continue;
        }
        return Ok(Some(line.to_string()));
    }
    return Ok(None);
}
pub fn parse_playlist(playlist: &Option<String>, mp4_header: &mut Option<Arc<Vec<u8>>>, username: &str, vr: bool) -> Result<Vec<Stream>> {
    let temp_dir = util::temp_dir().map_err(s!())?;
    util::create_dir(&temp_dir).map_err(s!())?;
    let mut streams = Vec::new();
    let mut date: Option<String> = None;
    if let Some(playlist) = playlist {
        for line in playlist.lines() {
            // parses MP4 header
            if mp4_header.is_none() {
                if line.contains("EXT-X-MAP:URI") {
                    let header_url_split = line.split("\"").collect::<Vec<&str>>();
                    if header_url_split.len() < 2 {
                        return Err("can not get mp4 header file".into());
                    }
                    let header_url = header_url_split[1];
                    let header = util::get_retry_vec(header_url, 5).map_err(s!())?;
                    *mp4_header = Some(Arc::new(header))
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
            if line.len() == 0 || &line[..1] == "#" {
                continue;
            }
            // parses relevant information
            let url = line.to_string();
            // parses stream id
            let id = line.split("_").last().ok_or_else(o!())?;
            let n = id.find(".").ok_or_else(o!())?;
            let id = (&id[..n]).trim().parse::<u32>().map_err(e!())?;
            let vr = if vr { "SCVR" } else { "SC" };
            let filename = match &date {
                Some(date) => {
                    format!("{}_{}_{}", vr, username, date)
                }
                None => break,
            };
            let filepath = format!("{}{}-{}-{}.mp4s", temp_dir, vr.to_lowercase(), username, id);
            streams.push(Stream::new(&filename, &url, id, &filepath, mp4_header.clone()));
        }
    }
    Ok(streams)
}
