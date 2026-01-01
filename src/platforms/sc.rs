use crate::{e, o, platforms::Platform, s, stream, util};
use std::{
    sync::{Arc, OnceLock},
    *,
};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
#[inline]
pub fn get_playlist(username: &str) -> Result<Option<String>> {
    sc_get_playlist(username, false)
}
#[inline]
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
    let mut playlist_url = None;
    for line in playlist.lines() {
        if line.len() < 5 || &line[..1] == "#" {
            continue;
        }
        playlist_url = Some(line.to_string());
    }
    if playlist.contains("EXT-X-MOUFLON") {
        for line in playlist.lines().rev() {
            if !line.contains("EXT-X-MOUFLON") {
                continue;
            }
            let segments: Vec<&str> = line.split(":").collect();
            let psch = segments.get(2).ok_or_else(o!())?;
            let pkey = segments.get(3).ok_or_else(o!())?;
            if let Some(url) = playlist_url {
                let playlist_url_append = format!("{}?&psch={}&pkey={}", url, psch, pkey);
                playlist_url = Some(playlist_url_append);
                break;
            }
        }
    }
    return Ok(playlist_url);
}

static PSCH: phf::Map<&'static str, &'static str> = phf::phf_map! {
    "Zokee2OhPh9kugh4" => "Quean4cai9boJa5a",
    "Ook7quaiNgiyuhai" => "EQueeGh2kaewa3ch",
};

static REGEX_ENCRY_TERM: OnceLock<Arc<regex::Regex>> = OnceLock::new();

pub fn sc_parse_playlist(playlist: &mut stream::Playlist, vr: bool) -> Result<Vec<stream::Stream>> {
    let platform = if vr { Platform::SCVR } else { Platform::SC };
    let temp_dir = util::temp_dir().map_err(s!())?;
    util::create_dir(&temp_dir).map_err(s!())?;
    let mut streams = Vec::new();
    let mut date: Option<String> = None;
    let mut key: Option<&str> = None;
    let iter: Vec<(usize, &str)> = playlist.playlist.as_ref().ok_or_else(o!())?.lines().enumerate().collect();
    for (n, line) in &iter {
        // get m3u8 encryption key
        if key.is_none() {
            if line.contains("#EXT-X-MOUFLON:PSCH") {
                let segments: Vec<&str> = line.split(":").collect();
                key = Some(PSCH[*(segments.get(3).ok_or_else(o!())?)])
            }
        }
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
                playlist.mp4_header = Some(sync::Arc::new(header))
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
        let url = if key.is_none() {
            line.to_string()
        } else {
            //// ENCRYPTED FILENAME
            // preprare key
            let key_bytes = key.ok_or_else(o!())?.as_bytes();
            use sha2::Digest;
            let mut sha256_hasher = sha2::Sha256::new();
            sha256_hasher.update(key_bytes);
            let key_bytes = sha256_hasher.finalize().to_vec();
            // preprare encrypted string
            let (_, mouflon) = &iter[n.saturating_sub(1)];
            let encoded_url = format!("https:{}", mouflon.split(":").last().ok_or_else(o!())?);
            // use stripchat's REGEX pattern to get encrypted string
            let re: &Arc<regex::Regex> =
                REGEX_ENCRY_TERM.get_or_init(|| regex::Regex::new(r"_([^_]+)_(\d+(?:_part\d+)?)\.mp4(?:[?#].*)?").unwrap().into());
            let re_captures = re.captures(&encoded_url).ok_or_else(o!())?;
            let encrypted_str = re_captures.get(1).ok_or_else(o!())?.as_str().to_string();
            // reverse string
            let mut encrypted_str_rev: String = encrypted_str.chars().rev().collect();
            if encrypted_str_rev.len() % 4 != 0 {
                for _ in 0..(4 - (encrypted_str_rev.len() % 4)) {
                    encrypted_str_rev.push('=');
                }
            }
            use base64::{Engine, engine::general_purpose::STANDARD};
            let mut encrypted_bytes = STANDARD.decode(encrypted_str_rev).map_err(e!())?;
            // XOR Decrypt
            let mut i = 0;
            while i < encrypted_bytes.len() {
                encrypted_bytes[i] = encrypted_bytes[i] ^ key_bytes[i % key_bytes.len()];
                i += 1;
            }
            let decrypted_str = String::from_utf8_lossy(&encrypted_bytes);
            println!("{}", String::from_utf8_lossy(&encrypted_bytes));
            // decrypted output
            encoded_url.replace(&encrypted_str, &decrypted_str)
        };
        // parse stream id
        let id = url.split("_").last().ok_or_else(o!())?;
        let n = id.find(".").ok_or_else(o!())?;
        let id = id[..n].trim().parse::<u32>().map_err(e!())?;
        // parse filename
        let vr_str = if vr { "SCVR" } else { "SC" };
        let date = date.as_ref().ok_or_else(o!())?;
        let filename = format!("{}_{}_{}", vr_str, playlist.username, date);
        let mut filepath = path::PathBuf::from(&temp_dir);
        filepath.push(format!("{}-{}-{}-{}.mp4", vr_str.to_lowercase(), playlist.username, date, id));
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
