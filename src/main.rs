use std::collections::HashMap;
use std::net::SocketAddr;
use std::os::fd::AsRawFd;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::{env, fs};

use ctl::Key::{self};
use ctl::json::PlaybackState;
use dotenv::dotenv;

use ctl::{
    Command, Connection, Handle, Query, Respond, Route, State, ThreadPool, client as spotify,
    websocket,
};
use epoll::ControlOptions::EPOLL_CTL_ADD;
use serde_json::json;
use tiny_http::SslConfig;

fn main() {
    dotenv().ok();

    let pi = env::var("DEVICE_ID").unwrap();
    let server_addr = env::var("SERVER_ADDR").unwrap();
    let soloist_addr = env::var("SOLOIST_ADDR").unwrap();
    let client_id = env::var("CLIENT_ID").unwrap();
    let client_secret = env::var("CLIENT_SECRET").unwrap();

    let client = spotify::Client::new(client_id, client_secret);
    let client2 = client.clone();

    let subscribers = Arc::new(Mutex::new(HashMap::<SocketAddr, Sender<String>>::new()));
    let subscribers2 = Arc::clone(&subscribers);

    let (ctlcmd_tx, ctlcmd_rx) = mpsc::channel();
    let ctlcmd_tx2 = ctlcmd_tx.clone();

    let server_handle = thread::spawn(move || {
        let pool = ThreadPool::new(10).expect("Failed to create thread pool");
        let server = tiny_http::Server::https(
            server_addr,
            SslConfig {
                certificate: fs::read("cert.pem").unwrap(),
                private_key: fs::read("key.pem").unwrap(),
            },
        )
        .expect("Failed to initialize server");

        for req in server.incoming_requests() {
            let client = client2.clone();
            let ctlcmd_tx = ctlcmd_tx2.clone();
            let subscribers = Arc::clone(&subscribers2);
            let pi = pi.clone();

            pool.execute(move || {
                let mut url_it = req.url().split("?");
                let path: &str = url_it.next().unwrap();
                let params: &str = url_it.next().unwrap_or_default();

                let start = State::new(req.method().as_str(), path, params, &req);

                let Some(routed) = start.route() else {
                    req.respond(
                        tiny_http::Response::from_string(include_str!(
                            "../templates/err/notfound.html"
                        ))
                        .with_status_code(404)
                        .with_header(
                            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..])
                                .unwrap(),
                        ),
                    )
                    .unwrap();
                    return;
                };

                let handled = routed.handle(
                    Box::new(move |query| match query {
                        Query::GetPlaylists(Key::Parsed(ids)) => client
                            .get_playlists(ids)
                            .map_err(|_| "failed to fetch playlists".into()),
                        Query::GetAvailableDevices(_) => match client.get_playback_state() {
                            Ok(s) if s.is_empty() => Ok(String::default()),
                            Ok(s) => {
                                let PlaybackState {
                                    device, is_playing, ..
                                } = serde_json::from_str(&s).unwrap();
                                (device.id == pi || !is_playing)
                                    .then_some(String::default())
                                    .ok_or("get your own spotify subscription, mom!".into())
                            }
                            Err(_) => Err("error fetching playback state".into()),
                        },
                        Query::IsConnected(_) => match client.get_playback_state() {
                            Ok(s) if s.is_empty() => Ok(String::default()),
                            Ok(s) => {
                                let PlaybackState { device, .. } =
                                    serde_json::from_str(&s).unwrap();
                                (device.id == pi && device.is_active)
                                    .then_some(String::default())
                                    .ok_or("device is not connected".into())
                            }
                            Err(_) => Err("failed to fetch playback state".into()),
                        },
                        Query::RequestAccessToken(Key::Parsed(auth_code)) => client
                            .authorize_with_auth_code(
                                auth_code.to_string(),
                                format!("{}/token", env::var("HOST").unwrap()),
                            )
                            .map_err(|_| "failed to authorize".into()),
                        _ => Err("could not match query".into()),
                    }),
                    Box::new(move |cmd| {
                        let maybe_cmd = match cmd {
                            Command::RegisterSubscriber(_) => Some(json!({
                                "type": "command",
                                "command": "get_state",
                            })),
                            Command::Activate(_) => Some(json!({
                                "type": "command",
                                "command": "activate",
                            })),
                            Command::PlayPlaylist(Key::Parsed(id)) => Some(json!({
                                "type": "command",
                                "command": "play",
                                "uri": format!("spotify:playlist:{id}"),
                            })),
                            Command::Play(_) => Some(json!({
                                "type": "command",
                                "command": "play",
                            })),
                            Command::Pause(_) => Some(json!({
                                "type": "command",
                                "command": "pause",
                            })),
                            _ => None,
                        };

                        let Some(cmd) = maybe_cmd else {
                            return Err("could not match command".into());
                        };

                        ctlcmd_tx
                            .send(cmd.to_string())
                            .map(|_| String::default())
                            .map_err(|_| "failed to send command to soloist".into())
                    }),
                );

                match handled {
                    Ok(handled) => {
                        let (status_code, headers, body, conn) = handled.respond();

                        match conn {
                            Connection::Close => {
                                let mut response = match body {
                                    Some(body) => tiny_http::Response::from_data(body.into_bytes()),
                                    None => tiny_http::Response::from_data(Vec::new()),
                                };
                                response = response.with_status_code(status_code);
                                headers.into_iter().for_each(|(k, v)| {
                                    response
                                        .add_header(tiny_http::Header::from_bytes(k, v).unwrap())
                                });
                                req.respond(response).unwrap();
                            }
                            Connection::Stall => {
                                let Some(&client_addr) = req.remote_addr() else {
                                    req.respond(
                                        tiny_http::Response::from_string(
                                            "only one connection allowed per client",
                                        )
                                        .with_status_code(403)
                                        .with_header(
                                            tiny_http::Header::from_bytes(
                                                &b"Content-Type"[..],
                                                &b"text/plain"[..],
                                            )
                                            .unwrap(),
                                        ),
                                    )
                                    .unwrap();
                                    return; // client has no addr
                                };

                                let mut http_response_raw = String::from("HTTP/1.1 200 OK\r\n");
                                for (k, v) in &headers {
                                    http_response_raw.push_str(&format!("{}: {}\r\n", k, v)[..]);
                                }
                                http_response_raw.push_str("\r\n");

                                let mut evstream = req.into_writer();
                                evstream
                                    .write_all(http_response_raw.as_bytes())
                                    .unwrap();
                                evstream.flush().unwrap_or_else(|_| {
                                    println!("HANDLER: error flushing SSE headers");
                                });

                                let (playbackev_tx, playbackev_rx) = mpsc::channel::<String>();

                                let mut subscribers_guard = subscribers.lock().unwrap();
                                if subscribers_guard.contains_key(&client_addr) {
                                    return; // one connection per address maximum
                                }
                                (*subscribers_guard).insert(client_addr, playbackev_tx);
                                drop(subscribers_guard);

                                while let Ok(json) =
                                    playbackev_rx.recv_timeout(Duration::from_mins(10))
                                {
                                    let event = "data: ".to_string() + &json + "\n\n";
                                    if evstream.write_all(event.as_bytes()).is_err() {
                                        println!("HANDLER: panicked on write to event stream");
                                        break;
                                    }
                                    // WARN: panics here
                                    if evstream.flush().is_err() {
                                        println!("HANDLER: panicked on flush to event stream");
                                        break;
                                    }
                                    if json.contains("\"type\": \"error\"") {
                                        break;
                                    }
                                }
                                (*subscribers.lock().unwrap()).remove(&client_addr);
                            }
                        }
                    }
                    Err((status_code, msg)) => {
                        req.respond(
                            tiny_http::Response::from_string(msg)
                                .with_status_code(tiny_http::StatusCode::from(status_code))
                                .with_header(
                                    tiny_http::Header::from_bytes(b"Content-Type", b"text/plain")
                                        .unwrap(),
                                ),
                        )
                        .unwrap();
                    }
                };
            });
        }
        println!("shutting down")
    });

    let soloistctl_handle = thread::spawn(move || {
        let ws_stream =
            ctl::websocket::connect(&soloist_addr).expect("could not connect to web socket");
        let ws_raw_fd = ws_stream.as_raw_fd();

        let epollfd = epoll::create(false).unwrap();
        let event = epoll::Event::new(epoll::Events::EPOLLIN, ws_raw_fd as u64);
        epoll::ctl(epollfd, EPOLL_CTL_ADD, ws_raw_fd, event).unwrap();

        thread::spawn(move || {
            let subscribers = Arc::clone(&subscribers);

            '_monitor_playback: loop {
                match websocket::recv_msg(ws_raw_fd, epollfd) {
                    Ok(msg) => {
                        subscribers
                            .lock()
                            .unwrap()
                            // WARN: panics here
                            .retain(|_, sub| {
                                let ok = sub.send(msg.clone()).is_ok();
                                if !ok {
                                    println!(
                                        "PLAYBACK_MONITOR: failed to send to subscribed handler"
                                    );
                                }
                                ok
                            })
                    }
                    Err(err) => match &err {
                        websocket::Error::ConnClosed => {
                            panic!("soloist has closed the websocket connection")
                        }
                        websocket::Error::Ping(payload) => {
                            websocket::send_pong(ws_raw_fd, payload.as_bytes()).unwrap()
                        }
                        _ => {
                            subscribers
                                .lock()
                                .unwrap()
                                .values()
                                .for_each(|sub| {
                                    sub.send(
                                        json!({
                                            "type": "error",
                                            "message": "receive error",
                                        })
                                        .to_string(),
                                    )
                                    .unwrap();
                                });
                        }
                    },
                }
            }
        });

        'monitor_cmds: loop {
            match ctlcmd_rx.recv() {
                Ok(cmd) => {
                    if cmd.is_empty() {
                        continue 'monitor_cmds;
                    }
                    websocket::send_msg(ws_raw_fd, cmd.as_bytes()).unwrap();
                }
                Err(e) => println!("soloist failed to receive command: {e}"),
            }
        }
    });

    server_handle.join().unwrap();
    soloistctl_handle.join().unwrap();
}
