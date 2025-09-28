#![allow(dead_code)]

use serde::de::DeserializeOwned;
use thiserror::Error;
use url::Url;

use crate::wasi::{
    http::{
        outgoing_handler::handle,
        types::{
            ErrorCode, Fields, HeaderError, IncomingBody, Method, OutgoingBody, OutgoingRequest,
            Scheme,
        },
    },
    io::streams::InputStream,
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("JSON serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Header error: {0}")]
    Header(#[from] HeaderError),

    #[error("HTTP error")]
    Http(#[from] ErrorCode),
}

impl From<Error> for ErrorCode {
    fn from(error: Error) -> Self {
        match error {
            Error::Http(error_code) => error_code,
            other => ErrorCode::InternalError(Some(other.to_string())),
        }
    }
}

pub struct Request {
    method: Method,
    url: Url,
    headers: Fields,
    body: Option<Vec<u8>>,
}

impl Request {
    pub fn new(method: Method, url: Url) -> Self {
        Self {
            method,
            url,
            headers: Fields::new(),
            body: None,
        }
    }

    pub fn with_headers(mut self, headers: Fields) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_header(self, name: &str, value: &str) -> Result<Self, Error> {
        self.headers.append(name, value.as_bytes())?;
        Ok(self)
    }

    pub fn with_body<T: Into<Vec<u8>>>(mut self, body: T) -> Result<Self, Error> {
        let body = body.into();
        let body_length = body.len();
        self.body = Some(body);
        self.with_header("Content-Length", &body_length.to_string())
    }

    pub fn with_json<T: serde::ser::Serialize>(self, body: &T) -> Result<Self, Error> {
        self.headers
            .append("Content-Type", b"application/json; charset=UTF-8")?;
        match serde_json::to_string(body) {
            Ok(json) => self.with_body(json),
            Err(err) => Err(Error::SerdeJson(err)),
        }
    }

    pub fn send(self) -> Result<Response, Error> {
        let response = self.execute_request(&self.url, &self.headers)?;

        if let Some(location) = response
            .headers
            .get("Location")
            .first()
            .and_then(|v| url::Url::parse(std::str::from_utf8(v).ok()?).ok())
        {
            self.execute_request(&location, &self.headers)
        } else {
            Ok(response)
        }
    }

    fn execute_request(&self, url: &Url, headers: &Fields) -> Result<Response, Error> {
        let req = OutgoingRequest::from_url(url, &self.method, headers.clone());

        if let Some(body) = &self.body {
            let outgoing_body = req.body().unwrap();
            let output_stream = outgoing_body.write().unwrap();
            for chunk in body.chunks(4096) {
                output_stream.blocking_write_and_flush(chunk).unwrap();
            }
            drop(output_stream);
            OutgoingBody::finish(outgoing_body, None).unwrap();
        }

        let inc_resp = handle(req, None)?;
        let sub_req = inc_resp.subscribe();
        sub_req.block();

        let incoming = inc_resp.get().unwrap().unwrap()?;
        let response = Response {
            status_code: incoming.status(),
            headers: incoming.headers().clone(),
            body: incoming.consume().unwrap(),
        };
        Ok(response)
    }
}

impl OutgoingRequest {
    pub fn from_url(url: &Url, method: &Method, headers: Fields) -> Self {
        let outgoing = OutgoingRequest::new(headers);
        outgoing.set_method(method).unwrap();
        let scheme = match url.scheme() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            _ => Scheme::Other(url.scheme().to_owned()),
        };
        outgoing.set_scheme(Some(&scheme)).unwrap();
        outgoing.set_authority(Some(url.authority())).unwrap();
        let path_with_query = match url.query() {
            Some(query) => format!("{}?{}", url.path(), query),
            None => url.path().to_string(),
        };
        outgoing
            .set_path_with_query(Some(&path_with_query))
            .unwrap();
        outgoing
    }
}

pub struct Response {
    pub status_code: u16,
    pub headers: Fields,
    body: IncomingBody,
}

impl Response {
    pub fn text(self) -> String {
        let full = self.bytes();
        let text = String::from_utf8_lossy(&full);
        text.into_owned()
    }

    pub fn bytes(self) -> Vec<u8> {
        let body_stream = self.body.stream().unwrap();

        let content_length = self
            .headers
            .get("Content-Length")
            .first()
            .and_then(|v| std::str::from_utf8(v).ok())
            .and_then(|v| v.parse::<u64>().ok());

        let read_limit = content_length.unwrap_or(u64::MAX);
        let mut full_bytes = Vec::with_capacity(content_length.unwrap_or(0) as usize);
        while let Ok(mut stream_bytes) = body_stream.blocking_read(read_limit) {
            full_bytes.append(stream_bytes.as_mut());
        }

        full_bytes
    }

    pub fn input_stream(self) -> InputStream {
        self.body.stream().unwrap()
    }

    pub fn json<T: DeserializeOwned>(self) -> Result<T, Error> {
        let full = self.bytes();
        let json = serde_json::from_slice(&full)?;
        Ok(json)
    }
}
