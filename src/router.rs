//! We can deduce all the possible response outcomes a request
//! can take by simply looking at the method and url.
//!
//! Hence, for any given request, the router will deduce:
//! - endpoint
//! - query
//! - fx command
//! - ok page
//!
use crate::data::{Command, Endpoint, Key, PLAYLISTS_ROOT, Page, Query};

pub fn get_endpoint(method: &str, path: &str) -> Option<Endpoint> {
    println!("in router: got endpoint {} {}", method, path);

    match (method, path) {
        ("GET", "/") => Some(Endpoint::Root),
        ("GET", "/playlist") => Some(Endpoint::GetPlaylist),
        ("GET", "/play-playlist") => Some(Endpoint::PlayPlaylist),
        ("GET", "/login") => Some(Endpoint::Login),
        ("GET", "/token") => Some(Endpoint::RegisterAccessToken),
        ("GET", "/script/index.js") => Some(Endpoint::GetScript),
        ("GET", "/subscribe") => Some(Endpoint::Subscribe),
        ("GET", "/activate") => Some(Endpoint::Activate),
        ("GET", "/play") => Some(Endpoint::Play),
        ("GET", "/pause") => Some(Endpoint::Pause),
        _ => None,
    }
}

pub fn get_query<'a>(endpoint: Endpoint, params: &'a str) -> Option<Query<'a>> {
    match endpoint {
        Endpoint::Root => Some(Query::GetPlaylists(Key::Raw(PLAYLISTS_ROOT))),
        Endpoint::GetPlaylist => Some(Query::GetPlaylist(Key::Raw(params))),
        Endpoint::RegisterAccessToken => Some(Query::RequestAccessToken(Key::Raw(params))),
        Endpoint::PlayPlaylist => Some(Query::IsConnected(Key::Raw(""))),
        Endpoint::Activate => Some(Query::GetAvailableDevices(Key::Raw(""))),
        Endpoint::Login => None,
        Endpoint::GetScript => None,
        Endpoint::Subscribe => None,
        Endpoint::Play => None,
        Endpoint::Pause => None,
    }
}

pub fn get_command<'a>(endpoint: Endpoint, params: &'a str) -> Option<Command<'a>> {
    match endpoint {
        Endpoint::Play => Some(Command::Play(Key::Raw(""))),
        Endpoint::Pause => Some(Command::Pause(Key::Raw(""))),
        Endpoint::PlayPlaylist => Some(Command::PlayPlaylist(Key::Raw(params))),
        Endpoint::Subscribe => Some(Command::RegisterSubscriber(Key::Raw(params))),
        Endpoint::Activate => Some(Command::Activate(Key::Raw(""))),
        Endpoint::Root => None,
        Endpoint::GetPlaylist => None,
        Endpoint::Login => None,
        Endpoint::RegisterAccessToken => None,
        Endpoint::GetScript => None,
    }
}

pub fn get_ok_page(endpoint: Endpoint) -> Page {
    match endpoint {
        Endpoint::Root => Page::Root,
        Endpoint::GetPlaylist => Page::Playlist,
        Endpoint::PlayPlaylist => Page::Playing,
        Endpoint::Login => Page::Login,
        Endpoint::RegisterAccessToken => Page::RequestAccessToken,
        Endpoint::GetScript => Page::Script,
        Endpoint::Subscribe => Page::EventsChan,
        Endpoint::Activate => Page::Connect,
        Endpoint::Play => Page::NoContent,
        Endpoint::Pause => Page::NoContent,
    }
}
