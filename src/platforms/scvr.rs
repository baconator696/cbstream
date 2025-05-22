use crate::platforms::sc;
use crate::stream::{Playlist, Stream};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
pub fn get_playlist(username: &str) -> Result<Option<String>> {
    sc::sc_get_playlist(username, false)
}
pub fn parse_playlist(playlist: &mut Playlist) -> Result<Vec<Stream>> {
    sc::sc_parse_playlist(playlist, true)
}
