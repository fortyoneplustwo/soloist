use std::{
    error::Error,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use base64::prelude::*;
use serde::{Deserialize, Serialize};
use ureq::{
    Body, SendBody,
    config::Config,
    http::{HeaderValue, StatusCode},
    middleware::{Middleware, MiddlewareNext},
};

/////////////////////
// Response types
/////////////////////

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    scope: String,
}

/////////////////////
// Body types
/////////////////////

#[derive(Serialize)]
struct TransferPlaybackBody<'a> {
    device_ids: Vec<&'a str>,
    play: bool,
}

//////////////////
// Token types
//////////////////

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct ClientCredentials {
    access_token: String,
    token_type: String,
    expires_in: u64,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct AuthorizationCode {
    access_token: String,
    token_type: String,
    expires_in: u64,
    scope: String,
    refresh_token: String,
}

//////////////////
// Shared state
//////////////////

struct AgentState {
    client_id: Arc<str>,
    client_secret: Arc<str>,
    expiry_time: Arc<Mutex<Option<Instant>>>,
    token_ptr: Arc<Mutex<Option<Token>>>,
}

#[derive(Clone, Debug)]
enum Token {
    ClientCredentials(ClientCredentials),
    AuthCode(AuthorizationCode),
}

//////////////////
// Middleware
//////////////////

struct AuthMiddleware(Arc<Mutex<AgentState>>);

impl Middleware for AuthMiddleware {
    fn handle(
        &self,
        mut request: ureq::http::Request<SendBody>,
        next: MiddlewareNext,
    ) -> Result<ureq::http::Response<Body>, ureq::Error> {
        // hold the lock for the entire refresh process
        let state = self.0.lock().unwrap();

        // return as early as possible if no refresh is needed
        let expiry_time = { *state.expiry_time.lock().unwrap() };
        let Some(expiry_time) = expiry_time else {
            return Err(ureq::Error::HostNotFound);
        };
        if Instant::now()
            < expiry_time
                .checked_sub(Duration::from_mins(5))
                .unwrap()
        {
            if let Some(token) = &*state.token_ptr.lock().unwrap() {
                let access_token = match token {
                    Token::ClientCredentials(token) => &token.access_token,
                    Token::AuthCode(token) => &token.access_token,
                };
                request.headers_mut().insert(
                    "Authorization",
                    HeaderValue::try_from(format!("Bearer {access_token}")).unwrap(),
                );
            }
            return next.handle(request);
        }

        // otherwise try refresh
        let mut token_guard = state.token_ptr.lock().unwrap();
        let Some(token) = &*token_guard else {
            return Err(ureq::Error::HostNotFound);
        };

        let (new_token, expiry_time) = match token {
            Token::ClientCredentials(_) => {
                let new_token = ureq::post("https://accounts.spotify.com/api/token")
                    .send_form([
                        ("grant_type", "client_credentials"),
                        ("client_id", &state.client_id),
                        ("client_secret", &state.client_secret),
                    ])?
                    .body_mut()
                    .read_json::<ClientCredentials>()?;

                let expiry_time = Instant::now()
                    .checked_add(Duration::from_secs(new_token.expires_in))
                    .unwrap();

                (Token::ClientCredentials(new_token), expiry_time)
            }
            Token::AuthCode(cur_token) => {
                let encoded =
                    BASE64_STANDARD.encode(format!("{}:{}", state.client_id, state.client_secret,));

                let RefreshResponse {
                    access_token,
                    token_type,
                    scope,
                    expires_in,
                } = ureq::post("https://accounts.spotify.com/api/token")
                    .header(
                        "Authorization",
                        HeaderValue::try_from(format!("Basic {encoded}")).unwrap(),
                    )
                    .send_form([
                        ("grant_type", "refresh_token"),
                        ("refresh_token", &cur_token.refresh_token),
                    ])?
                    .body_mut()
                    .read_json::<RefreshResponse>()?;

                let expiry_time = Instant::now()
                    .checked_add(Duration::from_secs(expires_in))
                    .unwrap();

                (
                    Token::AuthCode(AuthorizationCode {
                        access_token,
                        token_type,
                        scope,
                        expires_in,
                        refresh_token: cur_token.refresh_token.clone(),
                    }),
                    expiry_time,
                )
            }
        };

        token_guard.replace(new_token);
        drop(token_guard);

        state
            .expiry_time
            .lock()
            .unwrap()
            .replace(expiry_time);

        if let Some(token) = &*state.token_ptr.lock().unwrap() {
            let access_token = match token {
                Token::ClientCredentials(token) => &token.access_token,
                Token::AuthCode(token) => &token.access_token,
            };
            request.headers_mut().insert(
                "Authorization",
                HeaderValue::try_from(format!("Bearer {access_token}")).unwrap(),
            );
        }
        next.handle(request)
    }
}

//////////////////
// Constants
//////////////////

const WEB_API: &str = "https://api.spotify.com/v1/";

//////////////////
// Client API
//////////////////

#[derive(Clone)]
pub struct Client {
    agent: ureq::Agent,
    client_id: String,
    client_secret: String,
    token: Arc<Mutex<Option<Token>>>,
    expiry_time: Arc<Mutex<Option<Instant>>>,
}

impl Client {
    pub fn new(client_id: String, client_secret: String) -> Self {
        let token = Arc::new(Mutex::new(None));
        let expiry_time = Arc::new(Mutex::new(None));

        let state = AgentState {
            client_id: Arc::from(&client_id[..]),
            client_secret: Arc::from(&client_secret[..]),
            expiry_time: Arc::clone(&expiry_time),
            token_ptr: Arc::clone(&token),
        };

        let shared_state = Arc::new(Mutex::new(state));

        let config = Config::builder()
            .http_status_as_error(false)
            .middleware(AuthMiddleware(shared_state.clone()))
            .build();
        let agent = config.into();

        Client {
            agent,
            client_id,
            client_secret,
            token: Arc::clone(&token),
            expiry_time: Arc::clone(&expiry_time),
        }
    }

    pub fn authorize_with_client_credentials(&self) -> Result<(), Box<dyn Error>> {
        let token = ureq::post("https://accounts.spotify.com/api/token")
            .send_form([
                ("grant_type", "client_credentials"),
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
            ])?
            .body_mut()
            .read_json::<ClientCredentials>()?;

        let expiry_time = Instant::now()
            .checked_add(Duration::from_secs(token.expires_in))
            .unwrap();

        self.token
            .lock()
            .unwrap()
            .replace(Token::ClientCredentials(token));

        self.expiry_time
            .lock()
            .unwrap()
            .replace(expiry_time);

        Ok(())
    }

    pub fn authorize_with_auth_code(
        &self,
        code: String,
        redirect_url: String,
    ) -> Result<String, ureq::Error> {
        let credentials =
            BASE64_STANDARD.encode(format!("{}:{}", self.client_id, self.client_secret));

        let token = ureq::post("https://accounts.spotify.com/api/token")
            .header("Authorization", format!("Basic {credentials}"))
            .send_form([
                ("grant_type", "authorization_code"),
                ("redirect_uri", &redirect_url),
                ("code", &code),
            ])?
            .body_mut()
            .read_json::<AuthorizationCode>()?;

        let expiry_time = Instant::now()
            .checked_add(Duration::from_secs(token.expires_in))
            .unwrap();

        let refresh_token = token.refresh_token.clone();

        self.token
            .lock()
            .unwrap()
            .replace(Token::AuthCode(token));

        self.expiry_time
            .lock()
            .unwrap()
            .replace(expiry_time);

        Ok(refresh_token)
    }

    pub fn get_playlist(&self, id: &str) -> Result<String, ureq::Error> {
        self.agent
            .clone()
            .get(format!("{WEB_API}/playlists/{id}/items"))
            .call()?
            .body_mut()
            .read_to_string()
    }

    pub fn get_playlists(&self, ids: &str) -> Result<String, ureq::Error> {
        Ok(ids
            .split(",")
            .map(|id| {
                self.agent
                    .clone()
                    .get(format!("{WEB_API}/playlists/{id}"))
                    .call()?
                    .body_mut()
                    .read_to_string()
            })
            .collect::<Result<Vec<String>, ureq::Error>>()?
            .join("&"))
    }

    pub fn get_available_devices(&self) -> Result<String, ureq::Error> {
        self.agent
            .clone()
            .get(format!("{WEB_API}/me/player/devices"))
            .call()?
            .body_mut()
            .read_to_string()
    }

    pub fn get_playback_state(&self) -> Result<String, ureq::Error> {
        let mut response = self
            .agent
            .clone()
            .get(format!("{WEB_API}/me/player"))
            .call()?;

        if response.status() == StatusCode::from_u16(204).unwrap() {
            return Ok(String::default());
        }

        response.body_mut().read_to_string()
    }

    pub fn transfer_playback(&self, id: &str) -> Result<String, ureq::Error> {
        self.agent
            .clone()
            .put(format!("{WEB_API}/me/player"))
            .send_json(TransferPlaybackBody {
                device_ids: vec![id],
                play: true,
            })?
            .body_mut()
            .read_to_string()
    }
}
