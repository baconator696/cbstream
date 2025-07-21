pub mod bonga;
pub mod cb;
pub mod mfc;
pub mod sc;
pub mod scvr;
pub mod soda;

use crate::{
    h, o, s,
    stream::{Playlist, Stream},
};
use std::{
    sync::{Arc, RwLock},
    thread::JoinHandle,
    *,
};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
#[derive(Debug, Clone)]
pub enum Platform {
    CB,
    SC,
    SCVR,
    MFC,
    BONGA,
    SODA,
}
impl Platform {
    pub fn new(key: &str) -> Option<Self> {
        match key {
            "CB" => Some(Self::CB),
            "MFC" => Some(Self::MFC),
            "SC" => Some(Self::SC),
            "SCVR" => Some(Self::SCVR),
            "BONGA" => Some(Self::BONGA),
            "SODA" => Some(Self::SODA),
            _ => None,
        }
    }
    pub fn parse_playlist(&self) -> fn(&mut Playlist) -> Result<Vec<Stream>> {
        match self {
            Self::CB => cb::parse_playlist,
            Self::MFC => mfc::parse_playlist,
            Self::SC => sc::parse_playlist,
            Self::SCVR => scvr::parse_playlist,
            Self::BONGA => bonga::parse_playlist,
            Self::SODA => soda::parse_playlist,
        }
    }
    fn get_playlist(&self) -> fn(&str) -> Result<Option<String>> {
        match self {
            Self::CB => cb::get_playlist,
            Self::MFC => mfc::get_playlist,
            Self::SC => sc::get_playlist,
            Self::SCVR => scvr::get_playlist,
            Self::BONGA => bonga::get_playlist,
            Self::SODA => soda::get_playlist,
        }
    }
    pub fn referer(&self) -> &'static str {
        match self {
            Self::CB => "https://chaturbate.com/",
            Self::MFC => "https://www.myfreecams.com/",
            Self::SC => "https://stripchat.com/",
            Self::SCVR => "https://vr.stripchat.com/",
            Self::BONGA => "https://bongacams.com/",
            Self::SODA => "https://www.camsoda.com/",
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
    fn is_online(&mut self) -> bool {
        self.playlist_link = match self.platform.get_playlist()(&self.username) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                None
            }
        };
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
        let handle = thread::spawn(move || {
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
