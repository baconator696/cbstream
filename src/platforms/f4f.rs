use {
    crate::{config::Settings, e, o, platforms::Platform, s, stream, util},
    std::{sync::Arc, *},
};
type Res<T> = Result<T, Box<dyn error::Error>>;
pub fn get_playlist(
    username: &str,
    settings: Arc<Settings>,
) -> Res<(Option<String>, Option<String>)> {
    let headers = util::create_headers(serde_json::json!({
        "user-agent": &settings.user_agent,
        "referer": format!("{}{}",Platform::MFC.referer(),username),

    }))
    .map_err(s!())?;
    // get model id
    let url = "https://www.flirt4free.com/?tpl=index2&model=json";
    let mut json_raw = util::get_retry(&url, 1, Some(&headers)).map_err(s!())?;
    // parse json
    let json_start = json_raw.find("'models':").ok_or_else(o!())? + 10;
    let json_end = json_raw.find("'favorites':").ok_or_else(o!())?;
    let mut offset = 0;
    while let Some(bracket_pos) = json_raw
        .get(json_start + offset..json_end)
        .and_then(|s| s.find("}"))
    {
        offset = 1 + bracket_pos + offset;
    }
    json_raw = json_raw
        .get(json_start..json_start + offset)
        .ok_or_else(o!())?
        .to_string();
    json_raw.push(']');
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    // determine if model is online
    let online_models_value_option = json.as_array().and_then(|m| {
        m.iter().find_map(|model_value| {
            model_value
                .get("model_seo_name")
                .and_then(|model_name_val| model_name_val.as_str())
                .and_then(|model_name| {
                    (model_name == username.to_lowercase()).then_some(model_value)
                })
        })
    });
    // get model id
    let online_models_value = match online_models_value_option {
        Some(o) => o,
        None => return Ok((None, None)),
    };
    let model_id = online_models_value
        .get("model_id")
        .and_then(|v| v.as_str())
        .ok_or_else(o!())?;
    // get ws key
    let url = format!(
        "https://www.flirt4free.com/ws/rooms/chat-room-interface.php?a=login_room&model_id={}",
        model_id
    );
    let json_raw = util::get_retry(&url, 1, Some(&headers)).map_err(s!())?;
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let ws_token = json
        .get("token_enc")
        .and_then(|v| v.as_str())
        .ok_or_else(o!())?;
    let ws_port_tobe = json
        .get("config")
        .and_then(|v| v.get("room"))
        .and_then(|v| v.get("port_to_be"))
        .and_then(|v| v.as_str())
        .ok_or_else(o!())?;
    let ws_host = json
        .get("config")
        .and_then(|v| v.get("room"))
        .and_then(|v| v.get("host"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.split(".").next())
        .ok_or_else(o!())?;
    let ws_url = format!(
        "wss://www.flirt4free.com/{}/chat?token={}&port_to_be={}&model_id={}",
        ws_host, ws_token, ws_port_tobe, model_id
    );
    // get stream key from ws
    let (mut ws, _) = tungstenite::connect(ws_url).map_err(e!())?;
    let ws_json_raw = ws
        .read()
        .map_err(e!())?
        .into_text()
        .map_err(e!())?
        .to_string();
    ws.close(None)?;
    let ws_json: serde_json::Value = serde_json::from_str(&ws_json_raw).map_err(e!())?;
    let stream_key = ws_json
        .get("data")
        .and_then(|v| v.get("video_info"))
        .and_then(|v| v.get("hls"))
        .and_then(|v| v.get("providers"))
        .and_then(|v| v.as_array())
        .and_then(|vec| vec.iter().next())
        .and_then(|v| v.get("stream_key"))
        .and_then(|v| v.as_str())
        .ok_or_else(o!())?;
    let main_playlist_url = format!(
        "https://hls.vscdns.com/manifest.m3u8?key={}&model_id={}",
        stream_key, model_id
    );
    let main_playlist = util::get_retry(&main_playlist_url, 1, Some(&headers)).map_err(e!())?;
    for line in main_playlist.lines().rev() {
        if line.len() < 5 || &line[..1] == "#" {
            continue;
        }
        let playlist_link = format!(
            "{}/{}",
            util::url_prefix(&main_playlist_url, true).ok_or_else(o!())?,
            line
        );
        return Ok((Some(playlist_link), None));
    }
    return Ok((None, None));
}

pub fn parse_playlist(playlist: &mut stream::Playlist) -> Res<Vec<stream::Stream>> {
    let mut streams = Vec::new();
    for line in playlist.playlist.as_ref().ok_or_else(o!())?.lines() {
        if line.len() == 0 || &line[..1] == "#" {
            continue;
        }
        // parse relevant information
        // parse stream id
        let id = line
            .find(".ts")
            .and_then(|n| line.get(..n))
            .and_then(|s| s.split("_").last())
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(o!())?;
        //parse filenames
        let date = util::date();
        let filename = format!("F4F_{}_{}", playlist.username, date);
        streams.push(stream::Stream::new(
            &filename,
            line,
            None,
            id,
            Platform::F4F,
            playlist.settings.user_agent.clone(),
            None,
            None,
        ));
    }
    Ok(streams)
}
