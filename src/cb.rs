use crate::stream::{ManagePlaylist, ManageStream, Playlist, Stream};
use crate::{abort, config::ModelInfo, util};
use crate::{e, h, o, s};
use std::io::{Read, Seek, Write};
use std::sync::{Arc, RwLock};
use std::{collections::HashMap, thread::JoinHandle, *};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub struct Cb {
    username: String,
    playlist_link: Option<String>,
    thread_handle: Option<JoinHandle<()>>,
    abort: Arc<RwLock<bool>>,
}
impl Cb {
    fn new(username: &str) -> Self {
        Self {
            username: username.to_string(),
            playlist_link: None,
            thread_handle: None,
            abort: Arc::new(RwLock::new(false)),
        }
    }
    fn update_playlist(&mut self) -> Result<()> {
        let url = format!(
            "https://chaturbate.com/api/chatvideocontext/{}/",
            self.username
        );
        let json_raw = match util::get_retry(&url, 5).map_err(s!()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("username:'{}' probably doesn't exist\n{}", self.username, e);
                self.playlist_link = None;
                return Ok(());
            }
        };
        let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
        let playlist_url = json["hls_source"].as_str().unwrap();
        if playlist_url.len() == 0 {
            self.playlist_link = None;
            return Ok(());
        }
        let mut n = 0;
        loop {
            n = match str::find(&playlist_url[n + 1..], "/") {
                Some(m) => m + n + 1,
                None => break,
            };
        }
        if playlist_url.len() < n {
            return Ok(());
        }
        let prefix = &playlist_url[..n];
        let playlist = util::get_retry(&playlist_url, 5).map_err(s!())?;
        let mut split: Vec<&str> = playlist.split("\n").collect();
        split.reverse();
        for line in split {
            if line.len() < 5 || &line[..1] == "#" {
                continue;
            }
            self.playlist_link = Some(format!("{}/{}", prefix, line));
            break;
        }
        return Ok(());
    }
}
impl ModelInfo for Cb {
    fn is_online(&mut self) -> Result<bool> {
        self.update_playlist().map_err(s!())?;
        Ok(self.playlist_link.is_some())
    }
    fn is_finished(&self) -> bool {
        if let Some(h) = &self.thread_handle {
            h.is_finished()
        } else {
            true
        }
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
            let mut playlist = Playlist::new(u, p, a).map_err(s!()).unwrap();
            playlist.playlist().unwrap();
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
pub fn new(models: Option<&Vec<serde_json::Value>>) -> Option<HashMap<String, Box<dyn ModelInfo>>> {
    match models {
        Some(models) => {
            let mut map: HashMap<String, Box<dyn ModelInfo>> = HashMap::new();
            for model in models {
                if let Some(model) = model.as_str() {
                    map.insert(model.to_string(), Box::new(Cb::new(model)));
                }
            }
            if map.len() != 0 { Some(map) } else { None }
        }
        None => None,
    }
}
pub fn update(
    models: Option<&Vec<serde_json::Value>>,
    current: Option<&mut HashMap<String, Box<dyn ModelInfo>>>,
) -> Option<HashMap<String, Box<dyn ModelInfo>>> {
    let current = match current {
        Some(o) => o,
        None => return new(models),
    };
    let mut new_map: HashMap<String, Box<dyn ModelInfo>> = HashMap::new();
    match models {
        Some(models) => {
            for model in models {
                if let Some(model) = model.as_str() {
                    if new_map.contains_key(model) {
                        continue;
                    }
                    if current.contains_key(model) {
                        new_map.insert(model.to_string(), current.remove(model).unwrap());
                    } else {
                        new_map.insert(model.to_string(), Box::new(Cb::new(model)));
                    }
                }
            }
            if new_map.len() != 0 {
                Some(new_map)
            } else {
                None
            }
        }
        None => {
            if new_map.len() != 0 {
                Some(new_map)
            } else {
                None
            }
        }
    }
}

impl ManagePlaylist for Playlist {
    fn playlist(&mut self) -> Result<()> {
        while !*self.abort.read().map_err(s!())? && !abort::get().map_err(s!())? {
            if self.update_playlist().is_err() {
                break;
            }
            for stream in self.parse_playlist().map_err(s!())? {
                if let Some(last) = &self.last_stream {
                    if stream <= *last.read().map_err(s!())? {
                        continue;
                    }
                }
                let stream = Arc::new(RwLock::new(stream));
                let s = stream.clone();
                let l = self.last_stream.clone();
                thread::spawn(move || {
                    (*s.write().unwrap()).download(l).unwrap();
                });
                self.last_stream = Some(stream);
                thread::sleep(time::Duration::from_millis(500));
            }
            thread::sleep(time::Duration::from_millis(1500));
        }
        self.mux_streams()?;
        Ok(())
    }
    fn parse_playlist(&self) -> Result<Vec<Stream>> {
        let mut streams = Vec::new();
        let mut date: Option<String> = None;
        if let Some(playlist) = &self.playlist {
            for line in playlist.split("\n") {
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
                let url = format!("{}/{}", self.url_prefix().map_err(s!())?, line);
                let time = line.split("_").last().ok_or_else(o!())?;
                let n = time.find(".").ok_or_else(o!())?;
                let time = (&time[..n]).trim().parse::<u32>().map_err(e!())?;
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
                match fs::create_dir_all(&temp_dir) {
                    Err(e) => {
                        if e.kind() != io::ErrorKind::AlreadyExists {
                            return Err(e).map_err(e!())?;
                        }
                    }
                    _ => (),
                }
                let name = match &date {
                    Some(date) => {
                        format!("CB{}{}", self.username, date)
                    }
                    None => break,
                };
                let filepath = format!("{}{}.ts", temp_dir, time);
                println!("{}_{}", name, time);
                streams.push(Stream {
                    name,
                    url,
                    time,
                    filepath,
                    file: None,
                    last: None,
                });
            }
        }

        // todo
        Ok(streams)
    }
    fn mux_streams(&mut self) -> Result<()> {
        let mut streams: Vec<sync::Arc<sync::RwLock<Stream>>> = Vec::new();
        let mut last = match self.last_stream.take() {
            Some(o) => o,
            None => return Ok(()),
        };
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
        let slash = if cfg!(target_os = "windows") {
            "\\"
        } else {
            "/"
        };
        match fs::create_dir(&self.username) {
            Err(r) => {
                if r.kind() != io::ErrorKind::AlreadyExists {
                    return Err(r).map_err(s!())?;
                }
            }
            _ => (),
        };
        let filename = format!(
            "{}{}CB_{}.ts",
            &self.username,
            slash,
            streams[0].read().map_err(s!())?.name
        );
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(filename)
            .map_err(e!())?;
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
impl ManageStream for Stream {
    fn download(&mut self, last: Option<Arc<RwLock<Stream>>>) -> Result<()> {
        self.last = last;
        let data = util::get_retry_vec(&self.url, 5).map_err(s!())?;
        let mut file = fs::File::create_new(&self.filepath).map_err(e!())?;
        file.write_all(&data).map_err(e!())?;
        file.seek(io::SeekFrom::Start(0)).map_err(e!())?;
        self.file = Some(file);
        Ok(())
    }
}
