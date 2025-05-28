use crate::platform::Platform;
use crate::{e, o, s};
use crate::{stream, util};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub fn get_playlist(username: &str) -> Result<Option<String>> {
    let headers = util::create_headers(serde_json::json!({
        "user-agent": util::get_useragent().map_err(s!())?,
        "referer": format!("{}{}",Platform::MFC.referer(),username),

    }))
    .map_err(s!())?;
    let url = format!("https://api-edge.myfreecams.com/usernameLookup/{}", username);
    let json_raw = util::get_retry(&url, 5, Some(&headers)).map_err(s!())?;
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let id = match json["result"]["user"]["id"].as_i64() {
        Some(o) => o,
        None => {
            return Err("user not found").map_err(s!())?;
        }
    };
    let sessions = match json["result"]["user"]["sessions"].as_array() {
        Some(o) => o,
        None => return Ok(None),
    };
    if sessions.len() == 0 {
        return Ok(None);
    }
    let phase = sessions[0]["phase"].as_str().ok_or_else(o!())?;
    let playform_id = sessions[0]["platform_id"].as_i64().ok_or_else(o!())?;
    let server_name = sessions[0]["server_name"].as_str().ok_or_else(o!())?;
    let server_name = util::remove_non_num(server_name);
    let playlist_url = format!(
        "https://edgevideo.myfreecams.com/llhls/NxServer/{}/ngrp:mfc_{}{}{}.f4v_cmaf/playlist_sfm4s.m3u8",
        server_name, phase, playform_id, id
    );
    let playlist = util::get_retry(&playlist_url, 1, Some(&headers)).map_err(s!())?;
    for line in playlist.lines() {
        if line.len() < 5 || &line[..1] == "#" {
            continue;
        }
        let playlist_link = format!("{}/{}", util::url_prefix(&playlist_url).ok_or_else(o!())?, line);
        return Ok(Some(playlist_link));
    }
    return Ok(None);
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
        let id_split = line.split(".").collect::<Vec<&str>>();
        let id_raw = *id_split.get(id_split.len().saturating_sub(2)).ok_or_else(o!())?;
        let id = util::remove_non_num(id_raw).parse::<u32>().map_err(e!())?;
        //parse filenames
        let date = util::date();
        let filename = format!("MFC_{}_{}", playlist.username, date);
        let filepath = format!("{}mfc-{}-{}-{}.ts", temp_dir, playlist.username, date, id);
        streams.push(stream::Stream::new(&filename, &url, id, &filepath, None, Platform::MFC));
    }
    Ok(streams)
}
