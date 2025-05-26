use crate::platforms::{cb, mfc, sc, scvr};
use crate::stream::{Playlist, Stream};
use crate::{h, o, s};
use std::sync::{Arc, RwLock};
use std::{thread::JoinHandle, *};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
#[derive(Debug, Clone)]
pub enum Platform {
    CB,
    SC,
    SCVR,
    MFC,
}
impl Platform {
    pub fn parse_playlist(&self) -> fn(&mut Playlist) -> Result<Vec<Stream>> {
        match self {
            Self::CB => cb::parse_playlist,
            Self::MFC => mfc::parse_playlist,
            Self::SC => sc::parse_playlist,
            Self::SCVR => scvr::parse_playlist,
        }
    }
    pub fn new(key: &str) -> Option<Self> {
        match key {
            "CB" => Some(Self::CB),
            "MFC" => Some(Self::MFC),
            "SC" => Some(Self::SC),
            "SCVR" => Some(Self::SCVR),
            _ => None,
        }
    }
}
pub struct Model {
    platform: Platform,
    username: String,
    downloading: Arc<RwLock<bool>>,
    playlist_link: Option<String>,
    thread_handles: Vec<JoinHandle<()>>,
    abort: Arc<RwLock<bool>>,
}
impl Model {
    pub fn new(platform: Platform, username: &str) -> Self {
        Self {
            platform,
            username: username.to_string(),
            downloading: Arc::new(RwLock::new(false)),
            playlist_link: None,
            thread_handles: Vec::new(),
            abort: Arc::new(RwLock::new(false)),
        }
    }
    pub fn composite_key(&self) -> String {
        format!("{:?}:{}", self.platform, self.username)
    }
    /// downloads the latest playlist
    fn get_playlist(&mut self) {
        let response = match self.platform {
            Platform::CB => cb::get_playlist(&self.username),
            Platform::MFC => mfc::get_playlist(&self.username),
            Platform::SC => sc::get_playlist(&self.username),
            Platform::SCVR => scvr::get_playlist(&self.username),
        };
        self.playlist_link = match response {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                None
            }
        }
    }
    fn is_online(&mut self) -> bool {
        self.get_playlist();
        self.playlist_link.is_some()
    }
    fn is_downloading(&self) -> Result<bool> {
        Ok(*self.downloading.read().map_err(s!())?)
    }
    fn join_handles(&mut self) {
        let mut errors: Vec<String> = Vec::new();
        for handle in self.thread_handles.drain(..) {
            match handle.join().map_err(h!()) {
                Err(e) => errors.push(e),
                _ => (),
            };
        }
        for e in errors {
            eprintln!("{}", e)
        }
    }
    fn join_finished_handles(&mut self) -> Result<()> {
        let mut errors: Vec<String> = Vec::new();
        for handle in self.thread_handles.drain(..).collect::<Vec<JoinHandle<()>>>() {
            if handle.is_finished() {
                match handle.join().map_err(h!()) {
                    Err(e) => errors.push(e),
                    _ => (),
                };
            } else {
                self.thread_handles.push(handle);
            }
        }
        if errors.len() > 0 {
            let mut e = errors.remove(0);
            for error in errors {
                e = format!("{}\n{}", e, error)
            }
            return Err(e).map_err(s!())?;
        }
        Ok(())
    }
    /// main function for downloading a model
    pub fn download(&mut self) -> Result<()> {
        self.join_finished_handles().map_err(s!())?;
        if self.is_downloading().map_err(s!())? {
            return Ok(());
        }
        if self.is_online() {
            self.start_download_thread().map_err(s!())?;
        }
        Ok(())
    }
    fn start_download_thread(&mut self) -> Result<()> {
        let username = self.username.clone();
        let abort = self.abort.clone();
        let playlist_url = self.playlist_link.clone().ok_or_else(o!())?;
        let platform = self.platform.clone();
        let downloading = self.downloading.clone();
        *downloading.write().map_err(s!())? = true;
        let handle: thread::JoinHandle<()> = thread::spawn(move || {
            Playlist::new(platform, username, playlist_url, abort, downloading, None)
                .playlist()
                .unwrap();
        });
        self.thread_handles.push(handle);
        Ok(())
    }
    pub fn abort(&self) -> Result<()> {
        *self.abort.write().map_err(s!())? = true;
        Ok(())
    }
}
impl Drop for Model {
    fn drop(&mut self) {
        self.join_handles()
    }
}
