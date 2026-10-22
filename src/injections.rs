use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Image {
    pub url: String,
}

#[derive(Deserialize)]
pub struct Track {
    pub name: String,
    pub duration_ms: u32,
}

#[derive(Deserialize)]
pub struct PlaylistItem {
    pub item: Track,
}

#[derive(Deserialize)]
pub struct PlaylistItems {
    pub items: Vec<PlaylistItem>,
}

#[derive(Deserialize)]
pub struct Playlist {
    pub name: String,
    pub description: String,
    pub images: Vec<Image>,
    pub id: String,
}
