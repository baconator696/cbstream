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
        "user-agent": (&settings.user_agent).to_lowercase(),
        "referer": format!("{}{}",Platform::BONGA.referer(),username),
        "x-requested-with": "XMLHttpRequest",
    }))
    .map_err(s!())?;
    // get model playlist link
    let url = format!(
        "https://bongacams.com/tools/amf.php?t={}",
        time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .map_err(e!())?
            .as_secs()
    );
    let payload = format!(
        "method=getRoomData&args%5B%5D={}&args%5B%5D=&args%5B%5D=",
        username
    );
    let json_raw = util::post_retry(
        &url,
        1,
        Some(&headers),
        &payload,
        "application/x-www-form-urlencoded; charset=UTF-8",
    )
    .map_err(s!())?;
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let hls = match json
        .get("localData")
        .ok_or_else(o!())?
        .get("videoServerUrl")
        .ok_or_else(o!())?
        .as_str()
    {
        Some(o) => o,
        None => return Ok((None, None)),
    };
    let performer_data = json.get("performerData").ok_or_else(o!())?;
    if !performer_data
        .get("isOnline")
        .ok_or_else(o!())?
        .as_bool()
        .ok_or_else(o!())?
    {
        return Ok((None, None));
    }
    if performer_data
        .get("isAway")
        .ok_or_else(o!())?
        .as_bool()
        .ok_or_else(o!())?
    {
        return Ok((None, None));
    }
    let playlist_url = format!(
        "https:{}/hls/stream_{}/playlist.m3u8",
        hls,
        performer_data
            .get("username")
            .ok_or_else(o!())?
            .as_str()
            .ok_or_else(o!())?
    );
    // get playlist of resolutions
    let playlist = util::get_retry(&playlist_url, 1, Some(&headers)).map_err(s!())?;
    for line in playlist.lines().rev() {
        if line.len() < 5 || &line[..1] == "#" {
            continue;
        }
        let playlist_link = Some(format!(
            "{}/{}",
            util::url_prefix(&playlist_url, line).ok_or_else(o!())?,
            line
        ));
        return Ok((playlist_link, None));
    }
    Ok((None, None))
}
pub fn parse_playlist(playlist: &mut stream::Playlist) -> Res<Vec<stream::Stream>> {
    let mut streams = Vec::new();
    for line in playlist.playlist.as_ref().ok_or_else(o!())?.lines() {
        if line.len() == 0 || &line[..1] == "#" {
            continue;
        }
        // parse relevant information
        let url = format!(
            "{}/{}",
            util::url_prefix(&playlist.playlist_url, line).ok_or_else(o!())?,
            line
        );
        // parse stream id
        let id_split = line.split("_").collect::<Vec<&str>>();
        let id_raw = *id_split.get(3).ok_or_else(o!())?;
        let id = util::remove_non_num(id_raw).parse::<u32>().map_err(e!())?;
        //parse filenames
        let date = util::date();
        let filename = format!("BC_{}_{}", playlist.username, date);
        streams.push(stream::Stream::new(
            &filename,
            &url,
            None,
            id,
            Platform::BONGA,
            playlist.settings.user_agent.clone(),
            None,
            None,
        ));
    }
    Ok(streams)
}
