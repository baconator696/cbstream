use {
    crate::{
        abort,
        config::Settings,
        debug_eprintln, e, h, muxer, o,
        platforms::Platform,
        s,
        util::{self, ManagedFile},
    },
    std::{
        io::{Seek, Write},
        sync::{Arc, Mutex, RwLock, mpsc},
        *,
    },
};
type Res<T> = Result<T, Box<dyn error::Error>>;
#[derive(Clone)]
pub struct Playlist {
    pub platform: Platform,
    pub username: String,
    pub playlist_url: String,
    pub playlist_audio_url: Option<String>,
    pub playlist: Option<String>,
    pub playlist_audio: Option<String>,
    current_stream: Option<Arc<RwLock<Stream>>>,
    abort: Arc<RwLock<bool>>,
    downloading: Arc<RwLock<bool>>,
    /// video header
    pub mp4_header: Option<Arc<Vec<u8>>>,
    /// optional - for audio/video split streams
    pub mp4_header_audio: Option<Arc<Vec<u8>>>,
    pub settings: Arc<Settings>,
}
impl Playlist {
    pub fn new(
        platform: Platform,
        username: String,
        playlist_url: String,
        playlist_audio_url: Option<String>,
        abort: Arc<RwLock<bool>>,
        downloading: Arc<RwLock<bool>>,
        settings: Arc<Settings>,
    ) -> Self {
        Playlist {
            platform,
            username,
            playlist_url,
            playlist_audio_url,
            playlist: None,
            playlist_audio: None,
            current_stream: None,
            abort,
            downloading,
            mp4_header: None,
            mp4_header_audio: None,
            settings,
        }
    }
    /// updates downloaded playlist with url
    fn update_playlist(&mut self) -> Res<()> {
        let headers = util::create_headers(serde_json::json!({
            "user-agent": &self.settings.user_agent,
            "referer": self.platform.referer(),

        }))
        .map_err(s!())?;
        let playlist = util::get_retry(&self.playlist_url, 5, Some(&headers)).map_err(s!())?;
        self.playlist = Some(playlist);
        if let Some(playlist_audio_url) = &self.playlist_audio_url {
            let playlist_audio =
                util::get_retry(playlist_audio_url, 5, Some(&headers)).map_err(s!())?;
            self.playlist_audio = Some(playlist_audio);
        }
        Ok(())
    }
    fn abort_get(&self) -> Res<bool> {
        Ok(*self.abort.read().map_err(s!())?)
    }
    /// Main Playlist Loop
    pub fn playlist(&mut self) -> Res<()> {
        let d = self.downloading.clone();
        scopeguard::defer! {
            if let Ok(mut downloading) = d.write() {
                *downloading = false;
            }
        }
        let mut mux_thread: Option<thread::JoinHandle<Result<(), String>>> = None;
        let mut trys = 0;
        while !self.abort_get().map_err(s!())? && !abort::get().map_err(s!())? {
            if let Some(mux_thread) = mux_thread.as_ref() {
                if mux_thread.is_finished() {
                    break;
                }
            }
            if let Err(state) = self.update_playlist().map_err(s!()) {
                debug_eprintln!("{:?}:{}:{}", self.platform, self.username, state);
                break;
            }
            trys += 1;
            if trys > 20 {
                break;
            }
            for mut new_stream in self.parse_playlist() {
                if let Some(current) = &self.current_stream {
                    if new_stream <= *current.read().map_err(s!())? {
                        continue;
                    }
                }
                trys = 0;
                new_stream.index = match self.current_stream.as_ref() {
                    Some(o) => o.read().map_err(s!())?.index + 1,
                    None => 1,
                };
                let new_stream = Arc::new(RwLock::new(new_stream));
                if let Some(current_stream) = self.current_stream.as_ref() {
                    let mut guard = current_stream.write().map_err(s!())?;
                    guard.next_stream = Some(new_stream.clone());
                    guard.next_stream_yield_rx = None;
                    let _ = guard.next_stream_yield_tx.send(());
                }
                let s: Arc<RwLock<Stream>> = new_stream.clone();
                thread::spawn(move || {
                    download(s).unwrap();
                });
                self.current_stream = Some(new_stream);
                if mux_thread.is_none() {
                    let m = self.clone();
                    mux_thread = Some(thread::spawn(move || m.mux_streams().map_err(s!())));
                }
            }
            thread::sleep(time::Duration::from_secs(1));
        }
        // set muxer to finish up
        *self.downloading.write().map_err(s!())? = false;
        if let Some(current_stream) = self.current_stream.as_ref() {
            let mut guard = current_stream.write().map_err(s!())?;
            guard.next_stream_yield_rx = None;
            let _ = guard.next_stream_yield_tx.send(());
        }
        if let Some(mux_header) = mux_thread {
            mux_header.join().map_err(h!())?.map_err(s!())?;
        }
        Ok(())
    }
    fn parse_playlist(&mut self) -> Vec<Stream> {
        match self.platform.parse_playlist()(self) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                Vec::new()
            }
        }
    }
    fn mux_streams(mut self) -> Res<()> {
        let mux_id = util::unique_time().map_err(e!())?;
        let mut stream = self.current_stream.take().ok_or_else(o!())?;
        let temp_dir = util::temp_dir().map_err(s!())?;
        util::create_dir(&temp_dir).map_err(e!())?;
        // generate files from current stream and initializes it
        let mut repeat = false;
        'outer: loop {
            let (mut filename, contains_audio_bool) = {
                let stream_guard = stream.read().map_err(s!())?;
                (
                    stream_guard.filename.clone(),
                    stream_guard.url_audio.is_some(),
                )
            };
            if repeat {
                filename = format!("{}_{}", filename, mux_id)
            }
            let mut file: ManagedFile =
                ManagedFile::generate_filenames(&self.username, &filename, false).map_err(s!())?;
            let mut file_audio_option = if contains_audio_bool {
                let file = ManagedFile::generate_filenames(&self.username, &filename, true)
                    .map_err(s!())?;
                Some(file)
            } else {
                None
            };
            // determine if there is some space left in temp directory
            if let Some(available) = util::available_space_for_path(&file.path) {
                if available < 1 << 27 {
                    thread::sleep(time::Duration::from_secs(1));
                    continue 'outer;
                }
            }
            'inner: loop {
                // waits if current stream is still downloading
                let stream_downloading_yield = stream.read().map_err(s!())?.muxer_yield_rx.clone();
                if let Some(_yield) = stream_downloading_yield {
                    let _ = _yield.lock().map_err(s!())?.recv();
                }
                // write video stream
                let data_option = stream.read().map_err(s!())?.data.clone();
                if let Some(data) = data_option {
                    let pos = file.file.stream_position().map_err(e!())?;
                    if let Err(e) = file.file.write_all(&data) {
                        if e.kind() != io::ErrorKind::StorageFull {
                            return Err(e).map_err(e!())?;
                        }
                        file.file.seek(io::SeekFrom::Start(pos)).map_err(e!())?;
                        file.file.set_len(pos).map_err(e!())?;
                        break 'inner;
                    }
                }
                // write optional audiostream
                if let Some(file_audio) = file_audio_option.as_mut() {
                    let data_audio_option = stream.read().map_err(s!())?.data_audio.clone();
                    if let Some(data_audio) = data_audio_option {
                        let pos = file_audio.file.stream_position().map_err(e!())?;
                        if let Err(e) = file_audio.file.write_all(&data_audio) {
                            if e.kind() != io::ErrorKind::StorageFull {
                                Err(e).map_err(e!())?
                            }
                            file_audio
                                .file
                                .seek(io::SeekFrom::Start(pos))
                                .map_err(e!())?;
                            file.file.set_len(pos).map_err(e!())?;
                            break 'inner;
                        }
                    }
                }
                // gets next stream and quit if done
                let next_stream_yield = stream.read().map_err(s!())?.next_stream_yield_rx.clone();
                loop {
                    if let Some(next_stream_yield) = next_stream_yield.clone() {
                        if next_stream_yield
                            .lock()
                            .map_err(s!())?
                            .recv_timeout(time::Duration::from_secs(1))
                            .is_ok()
                        {
                            break;
                        }
                    } else {
                        break;
                    }
                    if stream.read().map_err(s!())?.next_stream.is_none() {
                        if !*self.downloading.read().map_err(s!())? {
                            break 'inner;
                        }
                    } else {
                        break;
                    }
                }
                let next_stream = match stream.write().map_err(s!())?.next_stream.take() {
                    Some(o) => o,
                    None => {
                        *self.downloading.write().map_err(s!())? = false;
                        break 'inner;
                    }
                };
                stream = next_stream;
            }
            repeat = true;
            // disables audio if it failed to download
            if let Some(file_audio) = file_audio_option.as_ref() {
                if file_audio.path.metadata().map_err(e!())?.len() == 0 {
                    file_audio_option = None;
                }
            }
            // skips muxing if nothing downloaded
            if file.path.metadata().map_err(e!())?.len() != 0 {
                muxer::muxer(file, file_audio_option, self.platform.clone()).map_err(s!())?;
            }
            if stream.read().map_err(s!())?.next_stream.is_none()
                && !*self.downloading.read().map_err(s!())?
            {
                break 'outer;
            }
        }
        return Ok(());
    }
}
pub struct Stream {
    pub filename: String,
    url: String,
    url_audio: Option<String>,
    stream_id: u32,
    index: u32,
    data: Option<Arc<Vec<u8>>>,
    data_audio: Option<Arc<Vec<u8>>>,
    pub mp4_header: Option<Arc<Vec<u8>>>,
    pub mp4_header_audio: Option<Arc<Vec<u8>>>,
    pub next_stream: Option<Arc<RwLock<Stream>>>,
    platform: Platform,
    user_agent: String,
    muxer_yield_tx: mpsc::Sender<()>,
    muxer_yield_rx: Option<Arc<Mutex<mpsc::Receiver<()>>>>,
    next_stream_yield_tx: mpsc::Sender<()>,
    next_stream_yield_rx: Option<Arc<Mutex<mpsc::Receiver<()>>>>,
}
impl Stream {
    pub fn new(
        filename: &str,
        url: &str,
        url_audio: Option<&str>,
        id: u32,
        platform: Platform,
        user_agent: String,
        mp4_header: Option<Arc<Vec<u8>>>,
        mp4_header_audio: Option<Arc<Vec<u8>>>,
    ) -> Self {
        let url_audio = if url_audio.is_none() {
            None
        } else {
            Some(url_audio.unwrap().to_string())
        };
        let (tx, rx) = mpsc::channel();
        let muxer_yield_tx = tx;
        let muxer_yield_rx = Some(Arc::new(Mutex::new(rx)));
        let (tx, rx) = mpsc::channel();
        let next_stream_yield_tx = tx;
        let next_stream_yield_rx = Some(Arc::new(Mutex::new(rx)));
        Self {
            filename: filename.to_string(),
            url: url.to_string(),
            url_audio: url_audio,
            stream_id: id,
            index: 0,
            data: None,
            data_audio: None,
            mp4_header,
            mp4_header_audio,
            next_stream: None,
            platform,
            user_agent,
            muxer_yield_tx,
            muxer_yield_rx,
            next_stream_yield_tx,
            next_stream_yield_rx,
        }
    }
}
/// downloads the stream given the Stream's url
fn download(stream_lock: Arc<RwLock<Stream>>) -> Res<()> {
    let stream_defer = stream_lock.clone();
    scopeguard::defer! {
        if let Ok(mut stream) = stream_defer.write().map_err(s!()) {
            stream.muxer_yield_rx = None;
            let _ = stream.muxer_yield_tx.send(());
        }
    }
    // quick read to not lock stream
    let stream = stream_lock.read().map_err(s!())?;
    let filename = stream.filename.clone();
    let stream_id = stream.stream_id.clone();
    let referer = stream.platform.referer();
    let url_video = stream.url.clone();
    let url_audio = stream.url_audio.clone();
    let user_agent = stream.user_agent.clone();
    let mp4_header = stream.mp4_header.clone();
    let mp4_header_audio = stream.mp4_header_audio.clone();
    drop(stream);
    println!("{}_{}", filename, stream_id);
    let headers = util::create_headers(serde_json::json!({
        "user-agent": user_agent,
        "referer": referer,
    }))
    .map_err(s!())?;
    let mut video_data: Vec<u8> =
        match util::get_retry_vec(&url_video, 5, Some(&headers)).map_err(s!()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}:{}", e, &url_video);
                return Ok(());
            }
        };
    if video_data.len() < 10000 {
        debug_eprintln!("{}", String::from_utf8_lossy(&video_data));
        return Ok(());
    }
    if let Some(mp4_header) = mp4_header {
        let mut video_combined = (*mp4_header).clone();
        video_combined.append(&mut video_data);
        video_data = video_combined;
    }
    let mut audio_data: Option<Arc<Vec<u8>>> = None;
    'audio: {
        if let Some(url_audio) = &url_audio {
            let mut audio_data_internal: Vec<u8> =
                match util::get_retry_vec(url_audio, 5, Some(&headers)).map_err(s!()) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("{}:{}", e, url_audio);
                        Vec::new()
                    }
                };
            if audio_data_internal.len() == 0 {
                break 'audio;
            };
            if let Some(mp4_header_audio) = mp4_header_audio {
                let mut audio_combined = (*mp4_header_audio).clone();
                audio_combined.append(&mut audio_data_internal);
                audio_data_internal = audio_combined;
            }
            audio_data = Some(Arc::new(audio_data_internal));
        }
    }
    let stream = &mut *stream_lock.write().map_err(s!())?;
    stream.data = Some(Arc::new(video_data));
    stream.data_audio = audio_data;
    Ok(())
}
impl PartialEq for Stream {
    fn eq(&self, other: &Self) -> bool {
        self.stream_id == other.stream_id
    }
}
impl PartialOrd for Stream {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.stream_id.cmp(&other.stream_id))
    }
}
