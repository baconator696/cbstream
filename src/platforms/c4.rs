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
    let url = format!(
        "https://www.cam4.com/rest/v1.0/profile/{}/streamInfo",
        username
    );
    let json_raw = util::get_retry(&url, 1, Some(&headers)).map_err(s!())?;
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    let main_playlist_url = json
        .get("cdnURL")
        .and_then(|v| v.as_str())
        .ok_or_else(o!())?;
    let main_playlist = util::get_retry(main_playlist_url, 1, Some(&headers)).map_err(s!())?;
    for line in main_playlist.lines() {
        if line.len() < 5 || &line[..1] == "#" {
            continue;
        }
        let playlist_link = format!(
            "{}/{}",
            util::url_prefix(&main_playlist_url, line).ok_or_else(o!())?,
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
        let filename = format!("C4_{}_{}", playlist.username, date);
        let url = format!(
            "{}/{}",
            util::url_prefix(&playlist.playlist_url, line).ok_or_else(o!())?,
            line
        );
        streams.push(stream::Stream::new(
            &filename,
            &url,
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
