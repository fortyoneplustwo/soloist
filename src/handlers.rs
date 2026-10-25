use crate::{
    Query, Routed,
    data::{Command, Endpoint, Key},
};

type Status = (u32, String);

// NOTE:
// if we do the dispatch in router.rs,
// then maybe we don't have to pass the raw request as cx
// to handler anymore
pub fn dispatch(
    endpoint: Endpoint,
) -> fn(cx: &tiny_http::Request, &mut Routed) -> Result<Status, Status> {
    match endpoint {
        Endpoint::Root => root,
        Endpoint::Login => login,
        Endpoint::GetPlaylist => get_playlist,
        Endpoint::PlayPlaylist => play_playlist,
        Endpoint::Play => no_content,
        Endpoint::Pause => no_content,
        Endpoint::RegisterAccessToken => register_token,
        Endpoint::Activate => transfer_playback,
        Endpoint::GetScript => no_op,
        Endpoint::Subscribe => no_op,
    }
}

//////////////////
//  Handlers
//////////////////
fn root(_cx: &tiny_http::Request, routed: &mut Routed) -> Result<Status, Status> {
    let Some(Query::GetPlaylists(Key::Raw(params))) = routed.query else {
        return Err((400, "No params found".to_string()));
    };

    let params: Vec<&str> = params.split("=").collect();
    if params.len() != 2 {
        return Err((400, "Not enough params".to_string()));
    }

    let (key, value) = (&params[0], &params[1]);
    if *key != "ids" {
        return Err((400, "Wrong key".to_string()));
    }
    if value.split(",").collect::<Vec<&str>>().is_empty() {
        return Err((400, "Wrong values".to_string()));
    }

    routed.query = Some(Query::GetPlaylists(Key::Parsed(value)));

    Ok((200, String::default()))
}

fn login(_cx: &tiny_http::Request, _routed: &mut Routed) -> Result<Status, Status> {
    Ok((302, String::default()))
}

fn register_token(_cx: &tiny_http::Request, routed: &mut Routed) -> Result<Status, Status> {
    let Some(Query::RequestAccessToken(Key::Raw(params))) = routed.query else {
        return Err((400, "No query params provided".into()));
    };

    let params = params.split("&").collect::<Vec<&str>>();
    if params.len() != 2 {
        println!("params of len {}: {:#?}", params.len(), params);
        return Err((400, "Wrong number of parameters".into()));
    };

    let (code, state) = (&params[0], &params[1]);

    let state = state.split("=").collect::<Vec<&str>>();
    let (k, v) = (&state[0], &state[1]);
    if *k != "state" || v.is_empty() {
        return Err((400, "Wrong params".into()));
    }

    let code = code.split("=").collect::<Vec<&str>>();
    let (k, v) = (&code[0], &code[1]);
    if *k == "error" {
        return Err((401, "Access denied".into()));
    }
    if *k != "code" || v.is_empty() {
        return Err((400, "Wrong params".into()));
    }

    routed.query = Some(Query::RequestAccessToken(Key::Parsed(v)));

    Ok((302, String::default()))
}

fn get_playlist(_cx: &tiny_http::Request, routed: &mut Routed) -> Result<Status, Status> {
    let Some(Query::GetPlaylist(Key::Raw(params))) = routed.query else {
        return Err((400, "Missing params".to_string()));
    };

    let params = params.split("=").collect::<Vec<&str>>();
    if params.len() != 2 {
        return Err((400, "Wrong number of params".to_string()));
    }
    let (k, v) = (&params[0], &params[1]);
    if *k != "id" || v.is_empty() {
        return Err((400, "Wrong params".to_string()));
    }

    routed.query = Some(Query::GetPlaylist(Key::Parsed(v)));

    Ok((200, String::default()))
}

fn play_playlist(_cx: &tiny_http::Request, routed: &mut Routed) -> Result<Status, Status> {
    println!("inside play_playlist handler");

    let Some(Command::PlayPlaylist(Key::Raw(params))) = routed.command else {
        return Err((400, "Missing params".into()));
    };

    println!("params: {params}");

    let params = params.split("=").collect::<Vec<&str>>();
    if params.len() != 2 {
        return Err((400, "Wrong number of params".into()));
    }
    let (k, v) = (&params[0], &params[1]);
    if *k != "id" || v.is_empty() {
        return Err((400, "Wrong params".into()));
    }

    routed.command = Some(Command::Play(Key::Parsed(v)));

    println!("params parsed: {v}");

    Ok((204, String::default()))
}

fn no_op(_cx: &tiny_http::Request, _routed: &mut Routed) -> Result<Status, Status> {
    Ok((200, String::default()))
}

fn no_content(_cx: &tiny_http::Request, _routed: &mut Routed) -> Result<Status, Status> {
    Ok((204, String::default()))
}

fn transfer_playback(_cx: &tiny_http::Request, _routed: &mut Routed) -> Result<Status, Status> {
    Ok((204, String::default()))
}
