use crate::platform::Platform;
use crate::{e, o, s};
use crate::{stream, util};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub fn get_playlist(username: &str) -> Result<Option<String>> {
    let headers = util::create_headers(serde_json::json!({
        "user-agent": util::get_useragent().map_err(s!())?.to_lowercase(),
        "referer": format!("{}{}",Platform::BONGA.referer(),username),
        "x-requested-with": "XMLHttpRequest",
    }))
    .map_err(s!())?;
    // get model playlist link
    let url = format!(
        "https://bongacams.com/tools/amf.php?t={}",
        time::SystemTime::now().duration_since(time::UNIX_EPOCH).map_err(e!())?.as_secs()
    );
    let payload = format!("method=getRoomData&args%5B%5D={}&args%5B%5D=&args%5B%5D=", username);
    let json_raw = util::post_retry(&url, 1, Some(&headers), &payload, "application/x-www-form-urlencoded; charset=UTF-8").map_err(s!())?;
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let hls = match json["localData"]["videoServerUrl"].as_str() {
        Some(o) => o,
        None => return Ok(None),
    };
    let playlist_url = format!(
        "https:{}/hls/stream_{}/playlist.m3u8",
        hls,
        json["performerData"]["username"].as_str().ok_or_else(o!())?
    );
    // get playlist of resolutions
    let playlist = util::get_retry(&playlist_url, 1, Some(&headers)).map_err(s!())?;
    let mut split = playlist.lines().collect::<Vec<&str>>();
    split.reverse();
    for line in split {
        if line.len() < 5 || &line[..1] == "#" {
            continue;
        }
        let playlist_link = Some(format!("{}/{}", util::url_prefix(&playlist_url).ok_or_else(o!())?, line));
        return Ok(playlist_link);
    }
    Ok(None)
}
pub fn parse_playlist(playlist: &mut stream::Playlist) -> Result<Vec<stream::Stream>> {
    let temp_dir = util::temp_dir().map_err(s!())?;
    util::create_dir(&temp_dir).map_err(s!())?;
    let mut streams = Vec::new();
    for line in playlist.playlist.as_ref().ok_or_else(o!())?.lines() {
        if line.len() == 0 || &line[..1] == "#" {
            continue;
        }
        // parse relevant information
        let url = format!("{}/{}", playlist.url_prefix().ok_or_else(o!())?, line);
        // parse stream id
        let id_split = line.split("_").collect::<Vec<&str>>();
        let id_raw = *id_split.get(3).ok_or_else(o!())?;
        let id = util::remove_non_num(id_raw).parse::<u32>().map_err(e!())?;
        //parse filenames
        let date = util::date();
        let filename = format!("BC_{}_{}", playlist.username, date);
        let filepath = format!("{}bc-{}-{}-{}.ts", temp_dir, playlist.username, date, id);
        streams.push(stream::Stream::new(&filename, &url, id, &filepath, None, Platform::BONGA));
    }
    Ok(streams)
}
