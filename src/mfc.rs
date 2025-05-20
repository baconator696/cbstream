use crate::stream::{self, ManagePlaylist, Playlist, Stream};
use crate::{abort, config::ModelActions, util};
use crate::{e, h, o, s};
use std::sync::{Arc, RwLock};
use std::{thread::JoinHandle, *};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub struct MfcModel {
    username: String,
    playlist_link: Option<String>,
    thread_handle: Option<JoinHandle<()>>,
    abort: Arc<RwLock<bool>>,
}
impl MfcModel {
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
        let url = format!("https://api-edge.myfreecams.com/usernameLookup/{}", self.username);
        let json_raw = match util::get_retry(&url, 5).map_err(s!()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                self.playlist_link = None;
                return Ok(());
            }
        };
        let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
        let id = match json["result"]["user"]["id"].as_i64() {
            Some(o) => o,
            None => {
                eprintln!("{}", Err::<(), &str>("user not found").map_err(s!()).unwrap_err());
                self.playlist_link = None;
                return Ok(());
            }
        };
        let sessions = match json["result"]["user"]["sessions"].as_array() {
            Some(o) => o,
            None => {
                self.playlist_link = None;
                return Ok(());
            }
        };
        if sessions.len() == 0 {
            self.playlist_link = None;
            return Ok(());
        }
        let phase = sessions[0]["phase"].as_str().ok_or_else(o!())?;
        let playform_id = sessions[0]["platform_id"].as_i64().ok_or_else(o!())?;
        let server_name = sessions[0]["server_name"].as_str().ok_or_else(o!())?;
        let server_name = util::remove_non_num(server_name);
        let playlist_url = format!(
            "https://edgevideo.myfreecams.com/llhls/NxServer/{}/ngrp:mfc_{}{}{}.f4v_cmaf/playlist_sfm4s.m3u8",
            server_name, phase, playform_id, id
        );
        let playlist = match util::get_retry(&playlist_url, 1).map_err(s!()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                self.playlist_link = None;
                return Ok(());
            }
        };
        for line in playlist.lines() {
            if line.len() < 5 || &line[..1] == "#" {
                continue;
            }
            self.playlist_link = Some(format!("{}/{}", util::url_prefix(&playlist_url).ok_or_else(o!())?, line));
            break;
        }
        return Ok(());
    }
}
impl ModelActions for MfcModel {
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
            MfcPlaylist::new(u, p, a).playlist().unwrap();
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
struct MfcPlaylist(Playlist);
impl MfcPlaylist {
    pub fn new(username: String, playlist_url: String, abort: Arc<RwLock<bool>>) -> Self {
        MfcPlaylist(Playlist::new(username, playlist_url, abort, None))
    }
}
impl ManagePlaylist for MfcPlaylist {
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
        if let Some(playlist) = &self.0.playlist {
            for line in playlist.lines() {
                if line.len() == 0 || &line[..1] == "#" {
                    continue;
                }
                // parses relevant information
                let url = format!("{}/{}", self.0.url_prefix().ok_or_else(o!())?, line);
                // parses stream id
                let id_split = line.split(".").collect::<Vec<&str>>();
                let id_raw = *id_split.get(id_split.len().saturating_sub(2)).ok_or_else(o!())?;
                let id = util::remove_non_num(id_raw).parse::<u32>().map_err(e!())?;
                let filename = format!("MFC_{}_{}", self.0.username, util::date());
                let filepath = format!("{}mfc-{}-{}.ts", temp_dir, self.0.username, id);
                streams.push(Stream::new(&filename, &url, id, &filepath, None));
            }
        }
        Ok(streams)
    }
    fn mux_streams(&mut self) -> Result<()> {
        stream::mux_streams(&mut self.0.last_stream, &self.0.username, "ts")
    }
}
