pub mod error;
pub mod middleware;

use url::Url;

use crate::{
    error::Error,
    middleware::{Middleware, Next},
    wasi::http::{
        outgoing_handler::handle,
        types::{Fields, Method, OutgoingBody, OutgoingRequest, Scheme},
    },
};

wit_bindgen::generate!({
    path: "../wit",
    world: "wasi:http/imports",
    generate_all,
});

pub struct RawRequest {
    method: Method,
    url: Url,
    headers: Fields,
    body: Option<Vec<u8>>,
}

impl RawRequest {
    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn method(&self) -> &Method {
        &self.method
    }

    fn send(self) -> Result<Response, Error> {
        let response = self.execute(&self.url)?;
        match response
            .headers
            .get("Location")
            .first()
            .and_then(|v| Url::parse(std::str::from_utf8(v).ok()?).ok())
        {
            Some(location) => self.execute(&location),
            None => Ok(response),
        }
    }

    fn execute(&self, url: &Url) -> Result<Response, Error> {
        let req = OutgoingRequest::new(self.headers.clone());
        req.set_method(&self.method).unwrap();
        req.set_scheme(Some(&match url.scheme() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            s => Scheme::Other(s.to_owned()),
        }))
        .unwrap();
        req.set_authority(Some(url.authority())).unwrap();
        req.set_path_with_query(Some(&match url.query() {
            Some(q) => format!("{}?{}", url.path(), q),
            None => url.path().to_string(),
        }))
        .unwrap();

        if let Some(body) = &self.body {
            let outgoing_body = req.body().unwrap();
            let stream = outgoing_body.write().unwrap();
            for chunk in body.chunks(4096) {
                stream.blocking_write_and_flush(chunk).unwrap();
            }
            drop(stream);
            OutgoingBody::finish(outgoing_body, None).unwrap();
        }

        let inc_resp = handle(req, None)?;
        inc_resp.subscribe().block();

        let incoming = inc_resp.get().unwrap().unwrap()?;
        let body = incoming.consume().unwrap();

        let body_stream = body.stream().unwrap();

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

        Ok(Response {
            status_code: incoming.status(),
            headers: incoming.headers().clone(),
            body: full_bytes,
        })
    }
}

pub struct Request<'m> {
    raw: RawRequest,
    middlewares: Vec<Box<dyn Middleware + 'm>>,
}

impl<'m> Request<'m> {
    pub fn new(method: Method, url: Url) -> Self {
        Self {
            raw: RawRequest {
                method,
                url,
                headers: Fields::new(),
                body: None,
            },
            middlewares: Vec::new(),
        }
    }

    pub fn headers(mut self, headers: Fields) -> Self {
        self.raw.headers = headers;
        self
    }

    pub fn header(self, name: &str, value: &str) -> Result<Self, Error> {
        self.raw.headers.append(name, value.as_bytes())?;
        Ok(self)
    }

    pub fn body<T: Into<Vec<u8>>>(mut self, body: T) -> Result<Self, Error> {
        let body = body.into();
        let body_length = body.len();
        self.raw.body = Some(body);
        self.header("Content-Length", &body_length.to_string())
    }

    #[cfg(feature = "json")]
    pub fn with_json<T: serde::ser::Serialize>(self, body: &T) -> Result<Self, Error> {
        self.raw
            .headers
            .append("Content-Type", b"application/json; charset=UTF-8")?;
        self.body(serde_json::to_string(body).map_err(Error::SerdeJson)?)
    }

    pub fn middleware(mut self, m: impl Middleware + 'm) -> Self {
        self.middlewares.push(Box::new(m));
        self
    }

    pub fn send(self) -> Result<Response, Error> {
        Next::new(&self.middlewares).run(self.raw)
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
    body: Vec<u8>,
}

impl Response {
    pub fn new(body: Vec<u8>) -> Self {
        Self {
            status_code: 200,
            headers: Fields::new(),
            body,
        }
    }

    pub fn as_str(&self) -> Result<&str, Error> {
        match str::from_utf8(&self.body) {
            Ok(s) => Ok(s),
            Err(err) => Err(Error::InvalidUtf8InBody(err)),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.body
    }

    pub fn into_body(self) -> Vec<u8> {
        self.body
    }

    #[cfg(feature = "json")]
    pub fn json<T: serde::de::DeserializeOwned>(self) -> Result<T, Error> {
        let str = self.as_str()?;
        let json = serde_json::from_str(str)?;
        Ok(json)
    }
}
