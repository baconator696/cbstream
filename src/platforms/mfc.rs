use crate::{e, o, s};
use crate::{stream, util};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub fn get_playlist(username: &str) -> Result<Option<String>> {
    let url = format!("https://api-edge.myfreecams.com/usernameLookup/{}", username);
    let json_raw = match util::get_retry(&url, 5).map_err(s!()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return Ok(None);
        }
    };
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let id = match json["result"]["user"]["id"].as_i64() {
        Some(o) => o,
        None => {
            eprintln!("{}", Err::<(), &str>("user not found").map_err(s!()).unwrap_err());
            return Ok(None);
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
    let playlist = match util::get_retry(&playlist_url, 1).map_err(s!()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return Ok(None);
        }
    };
    for line in playlist.lines() {
        if line.len() < 5 || &line[..1] == "#" {
            continue;
        }
        let playlist_link = Some(format!("{}/{}", util::url_prefix(&playlist_url).ok_or_else(o!())?, line));
        return Ok(playlist_link);
    }
    return Ok(None);
}
pub fn parse_playlist(playlist: &mut stream::Playlist) -> Result<Vec<stream::Stream>> {
    let temp_dir = util::temp_dir().map_err(s!())?;
    util::create_dir(&temp_dir).map_err(s!())?;
    let mut streams = Vec::new();
    if let Some(pl) = &playlist.playlist {
        for line in pl.lines() {
            if line.len() == 0 || &line[..1] == "#" {
                continue;
            }
            // parses relevant information
            let url = format!("{}/{}", playlist.url_prefix().ok_or_else(o!())?, line);
            // parses stream id
            let id_split = line.split(".").collect::<Vec<&str>>();
            let id_raw = *id_split.get(id_split.len().saturating_sub(2)).ok_or_else(o!())?;
            let id = util::remove_non_num(id_raw).parse::<u32>().map_err(e!())?;
            let filename = format!("MFC_{}_{}", playlist.username, util::date());
            let filepath = format!("{}mfc-{}-{}.ts", temp_dir, playlist.username, id);
            streams.push(stream::Stream::new(&filename, &url, id, &filepath, None));
        }
    }
    Ok(streams)
}
