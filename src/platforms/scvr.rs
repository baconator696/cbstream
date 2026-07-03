use {
    crate::{
        config::Settings,
        platforms::sc,
        stream::{Playlist, Stream},
    },
    std::{sync::Arc, *},
};
type Res<T> = Result<T, Box<dyn error::Error>>;
#[inline]
pub fn get_playlist(
    username: &str,
    settings: Arc<Settings>,
) -> Res<(Option<String>, Option<String>)> {
    sc::sc_get_playlist(username, true, settings)
}
#[inline]
pub fn parse_playlist(playlist: &mut Playlist) -> Res<Vec<Stream>> {
    sc::sc_parse_playlist(playlist, true)
}
