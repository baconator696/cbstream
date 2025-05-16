use crate::stream::{self, ManagePlaylist, Playlist, Stream};
use crate::{abort, config::ModelActions, sc};
use crate::{h, o, s};
use std::sync::{Arc, RwLock};
use std::{thread::JoinHandle, *};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub struct ScvrModel {
    username: String,
    playlist_link: Option<String>,
    thread_handle: Option<JoinHandle<()>>,
    abort: Arc<RwLock<bool>>,
}
impl ScvrModel {
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
        self.playlist_link = sc::get_playlist(&self.username, true).map_err(s!())?;
        Ok(())
    }
}
impl ModelActions for ScvrModel {
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
            ScvrPlaylist::new(u, p, a).playlist().unwrap();
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
struct ScvrPlaylist(Playlist);
impl ScvrPlaylist {
    pub fn new(username: String, playlist_url: String, abort: Arc<RwLock<bool>>) -> Self {
        ScvrPlaylist(Playlist::new(username, playlist_url, abort, None))
    }
}
impl ManagePlaylist for ScvrPlaylist {
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
        sc::parse_playlist(&self.0.playlist, &mut self.0.mp4_header, &self.0.username, true)
    }
    fn mux_streams(&mut self) -> Result<()> {
        stream::mux_streams(&mut self.0.last_stream, &self.0.username, "mp4")
    }
}
