use std::env;

use crate::{
    data::{AUTH_SCOPE, AUTH_STATE, Page},
    injections, templates,
};
use askama::Template;

pub enum Connection {
    Close,
    Stall,
}

type Response = (u32, Vec<(String, String)>, Option<String>, Connection);

pub enum InjectError {
    Deserialize(String),
    Render(String),
}

impl Page {
    pub fn build_response(
        self,
        status_code: u32,
        payload: String,
    ) -> Result<Response, InjectError> {
        match self {
            Page::Root => {
                let headers = vec![("Content-Type".into(), "text/html".into())];

                let injections::Playlist {
                    id,
                    name,
                    description,
                    mut images,
                    ..
                } = serde_json::from_str(&payload)
                    .map_err(|e| InjectError::Deserialize(e.to_string()))?;

                let body = templates::Root {
                    id,
                    name,
                    description,
                    image_url: std::mem::take(&mut images[0].url),
                }
                .render()
                .map_err(|e| InjectError::Render(e.to_string()))?;

                Ok((status_code, headers, Some(body), Connection::Close))
            }
            Page::Playlist => {
                let headers = vec![("Content-Type".into(), "text/html".into())];

                let injections::PlaylistItems { items: tracks, .. } =
                    serde_json::from_str(&payload)
                        .map_err(|e| InjectError::Deserialize(e.to_string()))?;

                let body = templates::Playlist { tracks }
                    .render()
                    .map_err(|e| InjectError::Render(e.to_string()))?;

                Ok((status_code, headers, Some(body), Connection::Close))
            }
            Page::Login => {
                let client_id = env::var("CLIENT_ID").unwrap();
                let headers = vec![
                    (
                        "Location".into(),
                        format!(
                            "\
                            https://accounts.spotify.com/authorize\
                            ?client_id={client_id}\
                                &response_type=code\
                                &redirect_uri={}/token\
                                &scope={AUTH_SCOPE}\
                                &state={AUTH_STATE}\
                            ",
                            env::var("HOST").unwrap()
                        ),
                    ),
                    (
                        "Set-Cookie".into(),
                        "oauth_state=; HttpOnly; SameSite=Lax; Path=/; Max-Age=600".into(),
                    ),
                    ("Cache-Control".into(), "no-store".into()),
                ];

                Ok((status_code, headers, None, Connection::Close))
            }
            Page::RequestAccessToken => Ok((
                status_code,
                vec![("Location".into(), env::var("HOST").unwrap())],
                None,
                Connection::Close,
            )),
            Page::Playing => Ok((status_code, Vec::default(), None, Connection::Close)),
            Page::Connect => Ok((status_code, Vec::default(), None, Connection::Close)),
            Page::Script => Ok((
                status_code,
                vec![("Content-Type".into(), "text/javascript".into())],
                Some(include_str!("../script/index.js").to_string()),
                Connection::Close,
            )),
            Page::EventsChan => {
                let headers = vec![
                    ("Content-Type".into(), "text/event-stream".into()),
                    ("Cache-Control".into(), "no-cache".into()),
                    ("Connection".into(), "keep-alive".into()),
                ];
                Ok((200, headers, None, Connection::Stall))
            }
        }
    }
}
