use crate::stream::{ManagePlaylist, ManageStream, Playlist, Stream};
use crate::{abort, config::ModelInfo, util};
use crate::{e, h, o, s};
use std::io::{Read, Seek, Write};
use std::sync::{Arc, RwLock};
use std::{thread::JoinHandle, *};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub struct Cb {
    username: String,
    playlist_link: Option<String>,
    thread_handle: Option<JoinHandle<()>>,
    abort: Arc<RwLock<bool>>,
}
impl Cb {
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
        let url = format!(
            "https://chaturbate.com/api/chatvideocontext/{}/",
            self.username
        );
        let json_raw = match util::get_retry(&url, 5).map_err(s!()) {
            Ok(r) => r,
            _ => {
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
        self.get_playlist().map_err(s!())?;
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
                // parses relevant information
                let url = format!("{}/{}", self.url_prefix().map_err(s!())?, line);
                // parses stream id
                let id = line.split("_").last().ok_or_else(o!())?;
                let n = id.find(".").ok_or_else(o!())?;
                let id = (&id[..n]).trim().parse::<u32>().map_err(e!())?;
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
                let filename = match &date {
                    Some(date) => {
                        format!("CB_{}_{}", self.username, date)
                    }
                    None => break,
                };
                let filepath = format!("{}{}-{}.ts", temp_dir, self.username, id);
                streams.push(Stream {
                    filename,
                    url,
                    time: id,
                    filepath,
                    file: None,
                    last: None,
                });
            }
        }
        Ok(streams)
    }
    fn mux_streams(&mut self) -> Result<()> {
        let mut streams: Vec<sync::Arc<sync::RwLock<Stream>>> = Vec::new();
        let mut last = match self.last_stream.take() {
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
        let slash = if cfg!(target_os = "windows") {
            "\\"
        } else {
            "/"
        };
        // creates output directory, places in current directory
        match fs::create_dir(&self.username) {
            Err(r) => {
                if r.kind() != io::ErrorKind::AlreadyExists {
                    return Err(r).map_err(s!())?;
                }
            }
            _ => (),
        };
        // creates filename
        let filename = format!(
            "{}{}{}.ts",
            &self.username,
            slash,
            streams[0].read().map_err(s!())?.filename
        );
        // creates file
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(filename)
            .map_err(e!())?;
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
impl ManageStream for Stream {
    fn download(&mut self, last: Option<Arc<RwLock<Stream>>>) -> Result<()> {
        println!("{}_{}", self.filename, self.time);
        self.last = last;
        let data = util::get_retry_vec(&self.url, 5).map_err(s!())?;
        let mut file = fs::File::create_new(&self.filepath).map_err(e!())?;
        file.write_all(&data).map_err(e!())?;
        file.seek(io::SeekFrom::Start(0)).map_err(e!())?;
        self.file = Some(file);
        Ok(())
    }
}
