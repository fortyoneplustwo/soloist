pub const PLAYLISTS_ROOT: &str = "ids=2U306z1sQfF0aUS2ofHNbb";
pub const AUTH_SCOPE: &str = "\
    playlist-read-private \
    user-read-playback-state \
    user-modify-playback-state";
pub const AUTH_STATE: &str = "abc123";

#[derive(Clone, Copy, Debug)]
pub enum Endpoint {
    Root,
    Login,
    RegisterAccessToken,
    GetPlaylist,
    PlayPlaylist,
    GetScript,
    Subscribe,
    Activate,
    Play,
    Pause,
}

#[derive(Clone, Copy, Debug)]
pub enum Key<'a> {
    Raw(&'a str),
    Parsed(&'a str),
}

#[derive(Clone, Copy, Debug)]
pub enum Query<'a> {
    GetPlaylist(Key<'a>),
    GetPlaylists(Key<'a>),
    RequestAccessToken(Key<'a>),
    GetAvailableDevices(Key<'a>),
    IsConnected(Key<'a>),
    TransferPlayback(Key<'a>),
}

#[derive(Clone, Copy, Debug)]
pub enum Command<'a> {
    PlayPlaylist(Key<'a>),
    Play(Key<'a>),
    Pause(Key<'a>),
    RegisterSubscriber(Key<'a>),
    Activate(Key<'a>),
}

#[derive(Clone, Copy, Debug)]
pub enum Page {
    Root,
    Login,
    RequestAccessToken,
    Playing,
    Playlist,
    Connect,
    Script,
    EventsChan,
    NoContent,
}
