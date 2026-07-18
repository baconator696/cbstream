pub mod bonga;
pub mod cb;
pub mod f4f;
pub mod mfc;
pub mod sc;
pub mod scvr;
pub mod soda;

use {
    crate::{
        config::Settings,
        h, o, s,
        stream::{Playlist, Stream},
    },
    std::{
        sync::{Arc, RwLock},
        thread::JoinHandle,
        *,
    },
};
type Res<T> = Result<T, Box<dyn error::Error>>;
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum Platform {
    CB,
    SC,
    SCVR,
    MFC,
    BONGA,
    SODA,
    F4F,
}
impl Platform {
    pub fn list() -> Vec<Platform> {
        use Platform::*;
        vec![CB, SC, SCVR, MFC, BONGA, SODA, F4F]
    }
    pub fn new(key: &str) -> Option<Self> {
        use Platform::*;
        match key {
            "CB" => Some(CB),
            "MFC" => Some(MFC),
            "SC" => Some(SC),
            "SCVR" => Some(SCVR),
            "BONGA" => Some(BONGA),
            "SODA" => Some(SODA),
            "F4F" => Some(F4F),
            _ => None,
        }
    }
    pub fn parse_playlist(&self) -> fn(&mut Playlist) -> Res<Vec<Stream>> {
        use Platform::*;
        match self {
            CB => cb::parse_playlist,
            MFC => mfc::parse_playlist,
            SC => sc::parse_playlist,
            SCVR => scvr::parse_playlist,
            BONGA => bonga::parse_playlist,
            SODA => soda::parse_playlist,
            F4F => f4f::parse_playlist,
        }
    }
    fn get_playlist(&self) -> fn(&str, Arc<Settings>) -> Res<(Option<String>, Option<String>)> {
        use Platform::*;
        match self {
            CB => cb::get_playlist,
            MFC => mfc::get_playlist,
            SC => sc::get_playlist,
            SCVR => scvr::get_playlist,
            BONGA => bonga::get_playlist,
            SODA => soda::get_playlist,
            F4F => f4f::get_playlist,
        }
    }
    pub fn referer(&self) -> &'static str {
        use Platform::*;
        match self {
            CB => "https://chaturbate.com/",
            MFC => "https://www.myfreecams.com/",
            SC => "https://stripchat.com/",
            SCVR => "https://vr.stripchat.com/",
            BONGA => "https://bongacams.com/",
            SODA => "https://www.camsoda.com/",
            F4F => "https://www.flirt4free.com/",
        }
    }
}
pub struct Model {
    pub platform: Platform,
    pub username: String,
    downloading: Arc<RwLock<bool>>,
    playlist_link: Option<String>,
    playlist_audio_link: Option<String>,
    thread_handles: Vec<JoinHandle<Result<(), String>>>,
    abort: Arc<RwLock<bool>>,
}
impl Model {
    pub fn new(platform: Platform, username: &str) -> Self {
        Self {
            platform,
            username: username.to_string(),
            downloading: Arc::new(RwLock::new(false)),
            playlist_link: None,
            playlist_audio_link: None,
            thread_handles: Vec::new(),
            abort: Arc::new(RwLock::new(false)),
        }
    }
    pub fn composite_key(&self) -> String {
        format!("{:?}:{}", self.platform, self.username)
    }
    fn is_online(&mut self, settings: Arc<Settings>) -> bool {
        let (playlist_link, playlist_audio_link) =
            match self.platform.get_playlist()(&self.username, settings) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("{}", e);
                    (None, None)
                }
            };
        self.playlist_audio_link = playlist_audio_link;
        self.playlist_link = playlist_link;
        self.playlist_link.is_some()
    }
    fn is_downloading(&self) -> Res<bool> {
        Ok(*self.downloading.read().map_err(s!())?)
    }
    fn join_handles_drop(&mut self) {
        let mut errors: Vec<String> = Vec::new();
        for handle in self.thread_handles.drain(..) {
            handle
                .join()
                .map_err(h!())
                .and_then(|r| r.map_err(s!()))
                .unwrap_or_else(|e| errors.push(e));
        }
        for e in errors {
            eprintln!("{}", e)
        }
    }
    fn join_finished_handles(&mut self) -> Res<()> {
        let mut errors = Vec::<String>::new();
        let handles: Vec<JoinHandle<Result<(), String>>> = self.thread_handles.drain(..).collect();
        for handle in handles {
            if handle.is_finished() {
                handle
                    .join()
                    .map_err(h!())
                    .and_then(|r| r.map_err(s!()))
                    .unwrap_or_else(|e| errors.push(e));
            } else {
                self.thread_handles.push(handle);
            }
            if errors.len() > 0 {
                return Err(errors.join("\n")).map_err(s!())?;
            }
        }
        return Ok(());
    }
    /// main function for downloading a model
    pub fn download(&mut self, settings: Arc<Settings>) -> Res<()> {
        self.join_finished_handles().map_err(s!())?;
        if self.is_downloading().map_err(s!())? {
            return Ok(());
        }
        if self.is_online(settings.clone()) {
            self.start_download_thread(settings).map_err(s!())?;
        }
        Ok(())
    }
    fn start_download_thread(&mut self, settings: Arc<Settings>) -> Res<()> {
        let username = self.username.clone();
        let abort = self.abort.clone();
        let playlist_url = self.playlist_link.clone().ok_or_else(o!())?;
        let playlist_audio_url = self.playlist_audio_link.clone();
        let platform = self.platform.clone();
        let settings = settings.clone();
        let downloading = self.downloading.clone();
        *downloading.write().map_err(s!())? = true;
        let handle = thread::spawn(move || {
            Playlist::new(
                platform,
                username,
                playlist_url,
                playlist_audio_url,
                abort,
                downloading,
                settings,
            )
            .playlist()
            .map_err(s!())
        });
        self.thread_handles.push(handle);
        Ok(())
    }
    pub fn abort(&self) -> Res<()> {
        *self.abort.write().map_err(s!())? = true;
        Ok(())
    }
}
impl hash::Hash for Model {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.platform.hash(state);
        self.username.hash(state);
    }
}
impl Eq for Model {}
impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        self.username == other.username
    }
}
impl Clone for Model {
    fn clone(&self) -> Self {
        Self::new(self.platform.clone(), &self.username)
    }
}
impl Drop for Model {
    fn drop(&mut self) {
        self.join_handles_drop()
    }
}
impl serde::Serialize for Model {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.username.clone())
    }
}
