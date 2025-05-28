use crate::platform::Platform;
use crate::{e, o, s};
use crate::{stream, util};
use std::sync::Arc;
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub fn get_playlist(username: &str) -> Result<Option<String>> {
    sc_get_playlist(username, false)
}
pub fn parse_playlist(playlist: &mut stream::Playlist) -> Result<Vec<stream::Stream>> {
    sc_parse_playlist(playlist, false)
}
pub fn sc_get_playlist(username: &str, vr: bool) -> Result<Option<String>> {
    let platform = if vr { Platform::SCVR } else { Platform::SC };
    let headers = util::create_headers(serde_json::json!({
        "user-agent": util::get_useragent().map_err(s!())?,
        "referer": format!("{}{}",platform.referer(),username),

    }))
    .map_err(s!())?;
    // get hls url prefix
    let url = "https://stripchat.com/api/front/models?primaryTag=girls";
    let json_raw = util::get_retry(url, 5, Some(&headers)).map_err(s!())?;
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let ref_hls = json["models"].as_array().ok_or_else(o!())?.get(0).ok_or_else(o!())?["hlsPlaylist"]
        .as_str()
        .ok_or_else(o!())?;
    let hls_prefix = ref_hls.split("/").collect::<Vec<&str>>().get(..3).ok_or_else(o!())?.join("/");
    // get model ID
    let url = format!("https://stripchat.com/api/front/v2/models/username/{}/cam", username);
    let json_raw = util::get_retry(&url, 5, Some(&headers)).map_err(s!())?;
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let model_id = json["user"]["user"]["id"].as_i64().ok_or_else(o!())?;
    // get largest HLS stream
    let vr = if vr { "_vr" } else { "" };
    let playlist_url = format!("{}/hls/{}{}/master/{}{}.m3u8", hls_prefix, model_id, vr, model_id, vr);
    // below is the transoded streams, (maybe add resolution settings in future)
    //let playlist_url = format!("{}/hls/{}_vr/master/{}_vr_auto.m3u8", hls_prefix, model_id, model_id);
    let playlist = match util::get_retry(&playlist_url, 1, Some(&headers)).map_err(s!()) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    for line in playlist.lines() {
        if line.len() < 5 || &line[..1] == "#" {
            continue;
        }
        return Ok(Some(line.to_string()));
    }
    return Ok(None);
}
pub fn sc_parse_playlist(playlist: &mut stream::Playlist, vr: bool) -> Result<Vec<stream::Stream>> {
    let platform = if vr { Platform::SCVR } else { Platform::SC };
    let temp_dir = util::temp_dir().map_err(s!())?;
    util::create_dir(&temp_dir).map_err(s!())?;
    let mut streams = Vec::new();
    let mut date: Option<String> = None;
    for line in playlist.playlist.as_ref().ok_or_else(o!())?.lines() {
        // parse MP4 header
        if playlist.mp4_header.is_none() {
            if line.contains("EXT-X-MAP:URI") {
                // parse header url
                let header_url_split = line.split("\"").collect::<Vec<&str>>();
                if header_url_split.len() < 2 {
                    return Err("can not get mp4 header file".into());
                }
                let header_url = header_url_split[1];
                let http_headers = util::create_headers(serde_json::json!({
                    "user-agent": util::get_useragent().map_err(s!())?,
                    "referer": platform.referer(),

                }))
                .map_err(s!())?;
                let header = util::get_retry_vec(header_url, 5, Some(&http_headers)).map_err(s!())?;
                playlist.mp4_header = Some(Arc::new(header))
            }
        }
        // parse date and time from playlist
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
        // parse relevant information
        let url = line.to_string();
        // parse stream id
        let id = line.split("_").last().ok_or_else(o!())?;
        let n = id.find(".").ok_or_else(o!())?;
        let id = (&id[..n]).trim().parse::<u32>().map_err(e!())?;
        // parse filename
        let vr_str = if vr { "SCVR" } else { "SC" };
        let date = date.as_ref().ok_or_else(o!())?;
        let filename = format!("{}_{}_{}", vr_str, playlist.username, date);
        let filepath = format!("{}{}-{}-{}-{}.mp4", temp_dir, vr_str.to_lowercase(), playlist.username, date, id);
        streams.push(stream::Stream::new(
            &filename,
            &url,
            id,
            &filepath,
            playlist.mp4_header.clone(),
            platform.clone(),
        ));
    }
    Ok(streams)
}
