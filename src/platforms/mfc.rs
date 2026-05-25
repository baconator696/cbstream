use crate::{e, o, platforms::Platform, s, stream, util};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub fn get_playlist(username: &str) -> Result<(Option<String>, Option<String>)> {
    let headers = util::create_headers(serde_json::json!({
        "user-agent": util::get_useragent().map_err(s!())?,
        "referer": format!("{}{}",Platform::MFC.referer(),username),

    }))
    .map_err(s!())?;
    let url = format!("https://api-edge.myfreecams.com/usernameLookup/{}", username);
    let json_raw = util::get_retry(&url, 5, Some(&headers)).map_err(s!())?;
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let user = json.get("result").ok_or_else(o!())?.get("user").ok_or_else(o!())?;
    let id = match user.get("id").ok_or_else(o!())?.as_i64() {
        Some(o) => o,
        None => {
            return Err("user not found").map_err(s!())?;
        }
    };
    let sessions = match user.get("sessions").ok_or_else(o!())?.as_array() {
        Some(o) => o,
        None => return Ok((None, None)),
    };
    if sessions.len() == 0 {
        return Ok((None, None));
    }
    let server_name = sessions[0].get("server_name").ok_or_else(o!())?.as_str().ok_or_else(o!())?;
    if server_name.len() == 0 {
        return Ok((None, None));
    }
    let phase = sessions[0].get("phase").ok_or_else(o!())?.as_str().ok_or_else(o!())?;
    let playform_id = sessions[0].get("platform_id").ok_or_else(o!())?.as_i64().ok_or_else(o!())?;
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
        let playlist_link = format!("{}/{}", util::url_prefix(&playlist_url, line).ok_or_else(o!())?, line);
        return Ok((Some(playlist_link), None));
    }
    return Ok((None, None));
}
pub fn parse_playlist(playlist: &mut stream::Playlist) -> Result<Vec<stream::Stream>> {
    let mut streams = Vec::new();
    for line in playlist.playlist.as_ref().ok_or_else(o!())?.lines() {
        if line.len() == 0 || &line[..1] == "#" {
            continue;
        }
        // parse relevant information
        let url = format!("{}/{}", util::url_prefix(&playlist.playlist_url, line).ok_or_else(o!())?, line);
        // parse stream id
        let id_split = line.split(".").collect::<Vec<&str>>();
        let id_raw = *id_split.get(1).ok_or_else(o!())?;
        let id = util::remove_non_num(id_raw).parse::<u32>().map_err(e!())?;
        //parse filenames
        let date = util::date();
        let filename = format!("MFC_{}_{}", playlist.username, date);
        streams.push(stream::Stream::new(&filename, &url, None, id, Platform::MFC));
    }
    Ok(streams)
}
