use askama::Template;
use serde::Deserialize;

use crate::injections;

#[derive(Deserialize, Template)]
#[template(path = "ok/index.html")]
pub struct Root {
    pub name: String,
    pub description: String,
    pub image_url: String,
    pub id: String,
}

#[derive(Deserialize, Template)]
#[template(path = "ok/playlist.html")]
pub struct Playlist {
    pub tracks: Vec<injections::PlaylistItem>,
}
