use crate::{
    platforms::sc,
    stream::{Playlist, Stream},
};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
#[inline]
pub fn get_playlist(username: &str) -> Result<Option<String>> {
    sc::sc_get_playlist(username, true)
}
#[inline]
pub fn parse_playlist(playlist: &mut Playlist) -> Result<Vec<Stream>> {
    sc::sc_parse_playlist(playlist, true)
}
