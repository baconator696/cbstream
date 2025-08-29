use crate::{e, o, platforms::Platform, s, stream, util};
use std::{
    sync::{Arc, OnceLock},
    *,
};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
static REGEX_GET: OnceLock<Arc<regex::Regex>> = OnceLock::new();
pub fn get_playlist(username: &str) -> Result<Option<String>> {
    let headers = util::create_headers(serde_json::json!({
        "user-agent": util::get_useragent().map_err(s!())?,
        "referer": format!("{}{}",Platform::SODA.referer(),username),

    }))
    .map_err(s!())?;
    // get model playlist link
    let url = format!("https://www.camsoda.com/{}", username);
    let html = util::get_retry(&url, 1, Some(&headers)).map_err(s!())?;
    let re: &Arc<regex::Regex> = REGEX_GET.get_or_init(|| regex::Regex::new(r#""stream":[^\}]+\}"#).unwrap().into());
    let json_string = re.find(&html).ok_or_else(o!())?.as_str();
    let json: serde_json::Value = serde_json::from_str(&format!("{{{}}}", json_string)).map_err(e!())?;
    let json = &json["stream"];
    let hostname_array = json["edge_servers"].as_array().ok_or_else(o!())?;
    if hostname_array.len() == 0 {
        return Ok(None);
    }
    let hostname = hostname_array[0].as_str().ok_or_else(o!())?;
    let stream_name = json["stream_name"].as_str().ok_or_else(o!())?;
    let token = json["token"].as_str().ok_or_else(o!())?;
    let playlist_url = format!(
        "https://{}/{}_v1/index.ll.m3u8?multitrack=true&filter=tracks:v4v3v2v1a1a2&token={}",
        hostname, stream_name, token
    );
    // get playlist of resolutions
    let playlist = util::get_retry(&playlist_url, 1, Some(&headers)).map_err(s!())?;
    for line in playlist.lines() {
        if line.len() < 5 || &line[..1] == "#" {
            continue;
        }
        return Ok(Some(line.into()));
    }
    Ok(None)
}
static REGEX_PARSE: OnceLock<Arc<regex::Regex>> = OnceLock::new();
pub fn parse_playlist(playlist: &mut stream::Playlist) -> Result<Vec<stream::Stream>> {
    let temp_dir = util::temp_dir().map_err(s!())?;
    util::create_dir(&temp_dir).map_err(s!())?;
    let mut streams = Vec::new();
    let mut date: Option<String> = None;
    for line in (playlist.playlist.as_ref()).ok_or_else(o!())?.lines() {
        // parse MP4 header
        if playlist.mp4_header.is_none() {
            if line.contains("EXT-X-MAP:URI") {
                // parse header url
                let header_url_split = line.split("\"").collect::<Vec<&str>>();
                if header_url_split.len() < 2 {
                    return Err("can not get mp4 header file".into());
                }
                let header_url = format!("{}/{}", playlist.url_prefix().ok_or_else(o!())?, header_url_split[1]);
                let http_headers = util::create_headers(serde_json::json!({
                    "user-agent": util::get_useragent().map_err(s!())?,
                    "referer": Platform::SODA.referer(),

                }))
                .map_err(s!())?;
                let header = util::get_retry_vec(&header_url, 5, Some(&http_headers)).map_err(s!())?;
                playlist.mp4_header = Some(sync::Arc::new(header))
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
        // parse stream id
        let re = REGEX_PARSE.get_or_init(|| regex::Regex::new(r"-(\d*).llhls.mp4").unwrap().into());
        let id = re
            .captures(line)
            .ok_or_else(o!())?
            .get(1)
            .ok_or_else(o!())?
            .as_str()
            .parse::<u32>()
            .map_err(e!())?;
        // parse filenames
        let date = date.as_ref().ok_or_else(o!())?;
        let filename = format!("CS_{}_{}", playlist.username, date);
        let mut filepath = path::PathBuf::from(&temp_dir);
        filepath.push(format!("cs-{}-{}-{}.mp4", playlist.username, date, id));
        streams.push(stream::Stream::new(
            &filename,
            line.into(),
            id,
            &filepath,
            playlist.mp4_header.clone(),
            Platform::SODA,
        ));
    }
    Ok(streams)
}
