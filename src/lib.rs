//! # Traits
//!
//! A server must implement the following traits:
//!
//! 1. Route
//! 2. Handle
//! 3. Respond

pub mod client;
mod data;
mod handlers;
mod injections;
mod pool;
mod responder;
mod router;
mod templates;
pub mod libc;
pub mod websocket;
pub mod json;

use std::error::Error;

pub use crate::pool::ThreadPool;
pub use data::{Command, Endpoint, Key, Query};
pub use responder::Connection;

//////////////////
//  Traits
//////////////////
pub trait Route {
    type Output: Handle;

    /// Returns an entity that implements the `Handle` trait.
    fn route(self) -> Option<Self::Output>;
}

pub trait Handle {
    type QueryFn;
    type FxFn;
    type T: Respond;
    type E;

    /// Returns an entity that implements the `Respond` trait.
    fn handle(self, query_fn: Self::QueryFn, fx_fn: Self::FxFn) -> Result<Self::T, Self::E>;
}

pub trait Respond {
    type Output;

    /// Takes care of serving content to the client.
    fn respond(self) -> Self::Output;
}

//////////////////
//  State
//////////////////
pub struct State<'a, S> {
    data: S,
    cx: &'a tiny_http::Request,
}

impl<'a> State<'a, Start<'a>> {
    pub fn new(
        method: &'a str,
        path: &'a str,
        params: &'a str,
        cx: &'a tiny_http::Request,
    ) -> State<'a, Start<'a>> {
        let start = Start {
            method,
            path,
            params,
        };
        State { data: start, cx }
    }
}

//////////////////
//  Type states
//////////////////
pub struct Start<'a> {
    method: &'a str,
    path: &'a str,
    params: &'a str,
}

#[derive(Debug)]
pub struct Routed<'a> {
    endpoint: data::Endpoint,
    query: Option<data::Query<'a>>,
    command: Option<data::Command<'a>>,
    page: data::Page,
}

pub struct Handled<T> {
    status_code: u32,
    payload: T,
    page: data::Page,
}

//////////////////
//  Impls
//////////////////
impl<'a, 'b> Route for State<'a, Start<'b>> {
    type Output = State<'a, Routed<'b>>;

    fn route(self) -> Option<Self::Output> {
        let endpoint = router::get_endpoint(self.data.method, self.data.path)?;
        let query = router::get_query(endpoint, self.data.params);
        let command = router::get_command(endpoint, self.data.params);
        let page = router::get_ok_page(endpoint);

        let next = Routed {
            endpoint,
            query,
            command,
            page,
        };

        Some(State {
            data: next,
            cx: self.cx,
        })
    }
}

impl<'a, 'b> Handle for State<'a, Routed<'b>> {
    type QueryFn = Box<dyn FnOnce(Query<'b>) -> Result<String, Box<dyn Error>>>;
    type FxFn = Box<dyn FnOnce(Command<'b>) -> Result<String, Box<dyn Error>>>;
    type T = State<'a, Handled<String>>;
    type E = (u32, String);

    fn handle(mut self, query_fn: Self::QueryFn, fx_fn: Self::FxFn) -> Result<Self::T, Self::E> {
        let handler_fn = handlers::dispatch(self.data.endpoint);
        let handler_result = handler_fn(self.cx, &mut self.data)?;

        let mut query_result = String::new();
        if let Some(key) = self.data.query {
            query_result = query_fn(key).map_err(|_| (500, "Spotify Web Api error".to_string()))?;
        };

        let mut fx_result = String::new();
        if let Some(cmd) = self.data.command {
            fx_result = fx_fn(cmd).map_err(|_| (500, "Spotify Soloist error".to_string()))?;
        };

        let final_result = handler_result.1 + "\n" + &query_result + "\n" + &fx_result;

        let next = Handled {
            status_code: handler_result.0,
            payload: final_result,
            page: self.data.page,
        };

        Ok(State {
            data: next,
            cx: self.cx,
        })
    }
}

impl<'a> Respond for State<'a, Handled<String>> {
    type Output = (u32, Vec<(String, String)>, Option<String>, Connection);

    fn respond(self) -> Self::Output {
        match self
            .data
            .page
            .build_response(self.data.status_code, self.data.payload)
        {
            Ok(res) => res,
            Err(e) => (
                500,
                vec![("Content-Type".into(), "text/plain".into())],
                match e {
                    responder::InjectError::Deserialize(e) => Some(e),
                    responder::InjectError::Render(e) => Some(e),
                },
                Connection::Close
            ),
        }
    }
}
