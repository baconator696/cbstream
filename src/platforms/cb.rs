use crate::platforms::Platform;
use crate::{e, o, s};
use crate::{stream, util};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub fn get_playlist(username: &str) -> Result<Option<String>> {
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
                return Ok(None);
            }
            Err(e)
        }
    }?;
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let playlist_url = json["hls_source"].as_str().ok_or_else(o!())?;
    if playlist_url.len() == 0 {
        return Ok(None);
    }
    // get playlist of resolutions
    let playlist = util::get_retry(&playlist_url, 1, Some(&headers)).map_err(s!())?;
    let mut split = playlist.lines().collect::<Vec<&str>>();
    split.reverse();
    for line in split {
        if line.len() < 5 || &line[..1] == "#" {
            continue;
        }
        let playlist_link = Some(format!("{}/{}", util::url_prefix(playlist_url).ok_or_else(o!())?, line));
        return Ok(playlist_link);
    }
    Ok(None)
}
pub fn parse_playlist(playlist: &mut stream::Playlist) -> Result<Vec<stream::Stream>> {
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
        let full_url = format!("{}/{}", playlist.url_prefix().ok_or_else(o!())?, line);
        // parse stream id
        let id = line.split("_").last().ok_or_else(o!())?;
        let n = id.find(".").ok_or_else(o!())?;
        let id = (&id[..n]).trim().parse::<u32>().map_err(e!())?;
        // parse filenames
        let date = date.as_ref().ok_or_else(o!())?;
        let filename = format!("CB_{}_{}", playlist.username, date);
        let filepath = format!("{}cb-{}-{}-{}.ts", temp_dir, playlist.username, date, id);
        streams.push(stream::Stream::new(&filename, &full_url, id, &filepath, None, Platform::CB));
    }

    Ok(streams)
}
