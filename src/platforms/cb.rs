use crate::{debug_eprintln, e, o, platforms::Platform, s, stream, util};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, OnceLock},
    *,
};
type Result<T> = result::Result<T, Box<dyn error::Error>>;

static REGEX_AUDIO_MATCH: OnceLock<Arc<regex::Regex>> = OnceLock::new();
static REGEX_AUDIO_MATCH2: OnceLock<Arc<regex::Regex>> = OnceLock::new();

pub fn get_playlist(username: &str) -> Result<(Option<String>, Option<String>)> {
    let username = username.to_lowercase();
    let headers = util::create_headers(serde_json::json!({
        "user-agent": util::get_useragent().map_err(s!())?,
        "referer": format!("{}{}",Platform::CB.referer(),username),

    }))
    .map_err(s!())?;
    // get model playlist link
    let url = format!("https://chaturbate.com/api/chatvideocontext/{}/", username);
    let json_raw = match util::get_retry(&url, 1, Some(&headers)).map_err(s!()) {
        Ok(r) => Ok(r),
        Err(e) => {
            if e.contains("Unauthorized") {
                debug_eprintln!("{}", e);
                return Ok((None, None));
            }
            Err(e)
        }
    }?;
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let playlist_url = json["hls_source"].as_str().ok_or_else(o!())?;
    if playlist_url.len() == 0 {
        return Ok((None, None));
    }
    // get playlist of resolutions
    let playlist = util::get_retry(&playlist_url, 1, Some(&headers)).map_err(s!())?;
    let playlist_audio_url = if playlist.contains("audio") {
        let re: &Arc<regex::Regex> = REGEX_AUDIO_MATCH.get_or_init(|| regex::Regex::new(r#"audio_aac_128.*?URI="([^"]*)""#).unwrap().into());
        if let Some(captures) = re.captures(&playlist) {
            if let Some(match_) = captures.get(1) {
                let audio_uri = match_.as_str();
                let audio_url = format!("{}{}", util::url_prefix(playlist_url, &audio_uri).ok_or_else(o!())?, audio_uri);
                Some(audio_url)
            } else {
                None
            }
        } else {
            let re: &Arc<regex::Regex> = REGEX_AUDIO_MATCH2.get_or_init(|| regex::Regex::new(r#"audio_aac_96.*?URI="([^"]*)""#).unwrap().into());
            if let Some(captures) = re.captures(&playlist) {
                if let Some(match_) = captures.get(1) {
                    let audio_uri = match_.as_str();
                    let audio_url = format!("{}{}", util::url_prefix(playlist_url, &audio_uri).ok_or_else(o!())?, audio_uri);
                    Some(audio_url)
                } else {
                    None
                }
            } else {
                None
            }
        }
    } else {
        None
    };
    for line in playlist.lines().rev() {
        if line.len() < 5 || &line[..1] == "#" {
            continue;
        }
        let playlist_url = Some(format!("{}{}", util::url_prefix(playlist_url, line).ok_or_else(o!())?, line));
        return Ok((playlist_url, playlist_audio_url));
    }
    Ok((None, None))
}
pub fn parse_playlist(playlist: &mut stream::Playlist) -> Result<Vec<stream::Stream>> {
    if playlist.playlist_audio_url.is_some() {
        return combine_playlist_audio_video(playlist);
    }
    let temp_dir = util::temp_dir().map_err(s!())?;
    util::create_dir(&temp_dir).map_err(s!())?;
    let mut streams = Vec::new();
    let mut date: Option<String> = None;
    for line in (playlist.playlist.as_ref()).ok_or_else(o!())?.lines() {
        // parse date and time
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
        let full_url = format!("{}/{}", util::url_prefix(&playlist.playlist_url, line).ok_or_else(o!())?, line);
        // parse stream id
        let id = line.split("_").last().ok_or_else(o!())?;
        let n = id.find(".").ok_or_else(o!())?;
        let id = (&id[..n]).trim().parse::<u32>().map_err(e!())?;
        // parse filenames
        let date = date.as_ref().ok_or_else(o!())?;
        let filename = format!("CB_{}_{}", playlist.username, date);
        let mut filepath = path::PathBuf::from(&temp_dir);
        filepath.push(format!("cb-{}-{}-{}.ts", playlist.username, date, id));
        streams.push(stream::Stream::new(&filename, &full_url, None, id, &filepath, None, None, Platform::CB));
    }

    Ok(streams)
}

fn combine_playlist_audio_video(playlist: &mut stream::Playlist) -> Result<Vec<stream::Stream>> {
    let temp_dir = util::temp_dir().map_err(s!())?;
    util::create_dir(&temp_dir).map_err(s!())?;
    let mut streams = Vec::new();
    let video_streams = parse_playlist_audio_video(playlist, &temp_dir, false).map_err(s!())?;
    let audio_streams = parse_playlist_audio_video(playlist, &temp_dir, true).map_err(s!())?;
    let mut keys: Vec<_> = video_streams.keys().collect();
    keys.sort();
    for id in keys {
        if audio_streams.contains_key(&id) {
            let filename = video_streams.get(&id).unwrap().filename.as_str();
            let video_url = video_streams.get(&id).unwrap().url.as_str();
            let audio_url = Some(audio_streams.get(&id).unwrap().url.as_str());
            let filepath = video_streams.get(&id).unwrap().filepath.as_ref();
            let new_stream = stream::Stream::new(
                filename,
                video_url,
                audio_url,
                *id,
                filepath,
                playlist.mp4_header.clone(),
                playlist.mp4_header_audio.clone(),
                Platform::CB,
            );
            streams.push(new_stream);
        }
    }
    Ok(streams)
}
struct Info {
    url: String,
    filepath: PathBuf,
    filename: String,
}
fn parse_playlist_audio_video(playlist: &mut stream::Playlist, temp_dir: &PathBuf, audio: bool) -> Result<HashMap<u32, Info>> {
    let mut date: Option<String> = None;
    let mut streams: HashMap<u32, Info> = HashMap::new();
    let playlist_text = if audio {
        playlist.playlist_audio.as_ref().ok_or_else(o!())?
    } else {
        playlist.playlist.as_ref().ok_or_else(o!())?
    };
    let playlist_mp4_header = if audio {
        &mut playlist.mp4_header_audio
    } else {
        &mut playlist.mp4_header
    };
    let playlist_url: &str = if audio {
        playlist.playlist_audio_url.as_ref().ok_or_else(o!())?
    } else {
        playlist.playlist_url.as_ref()
    };
    for line in playlist_text.lines() {
        // parse MP4 header
        if playlist_mp4_header.is_none() {
            if line.contains("EXT-X-MAP:URI") {
                // parse header url
                let header_url_split = line.split("\"").collect::<Vec<&str>>();
                if header_url_split.len() < 2 {
                    return Err("can not get mp4 header file".into());
                }
                let header_url = format!(
                    "{}{}",
                    util::url_prefix(playlist_url, header_url_split[1]).ok_or_else(o!())?,
                    header_url_split[1]
                );
                let http_headers = util::create_headers(serde_json::json!({
                    "user-agent": util::get_useragent().map_err(s!())?,
                    "referer": Platform::CB.referer(),

                }))
                .map_err(s!())?;
                let header = util::get_retry_vec(&header_url, 5, Some(&http_headers)).map_err(s!())?;
                *playlist_mp4_header = Some(sync::Arc::new(header))
            }
        }
        // parse date and time
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
        let full_url = format!("{}{}", util::url_prefix(&playlist.playlist_url, line).ok_or_else(o!())?, line);
        // parse stream id
        let id = line.split("/").last().ok_or_else(o!())?.split("_").collect::<Vec<_>>();
        let id2 = id.get(2).ok_or_else(o!())?;
        let id = id2.trim().parse::<u32>().map_err(e!())?;
        // parse filenames
        let date = date.as_ref().ok_or_else(o!())?;
        let filename = format!("CB_{}_{}", playlist.username, date);
        let mut filepath = path::PathBuf::from(&temp_dir);
        filepath.push(format!("cb-{}-{}-{}.ts", playlist.username, date, id));
        let stream = Info {
            url: full_url,
            filepath,
            filename,
        };
        streams.insert(id, stream);
    }
    Ok(streams)
}
