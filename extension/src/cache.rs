use crate::{
    nero::keyvalue_ttl::{self, store::Bucket},
    wasi::logging::logging::{self, Level},
};
use request::{
    RawRequest, Response,
    error::Error,
    middleware::{Middleware, Next},
};

const DEFAULT_CACHE_TTL_MS: u32 = 3_600_000;

pub struct CacheMiddleware {
    bucket: Bucket,
    ttl_ms: Option<u32>,
}

impl Default for CacheMiddleware {
    fn default() -> Self {
        Self {
            bucket: keyvalue_ttl::store::open("").unwrap(),
            ttl_ms: None,
        }
    }
}

impl Middleware for CacheMiddleware {
    fn handle(&self, req: RawRequest, next: Next<'_>) -> Result<Response, Error> {
        let cache_key = req.url().to_string();

        if let Some(cached) = self.bucket.get(&cache_key).unwrap() {
            return Ok(Response::new(cached));
        }

        let response = next.run(req)?;
        let ttl = self
            .ttl_ms
            .or_else(|| {
                parse_max_age(response.headers.get("Cache-Control").first()?).filter(|ttl| *ttl > 0)
            })
            .unwrap_or(DEFAULT_CACHE_TTL_MS);

        let status_code = response.status_code;
        let headers = response.headers.clone();
        let body = response.into_body();

        if let Err(err) = self.bucket.set(&cache_key, &body, Some(ttl)) {
            logging::log(
                Level::Warn,
                "cache-middleware",
                &format!(
                    "cache set failed (key='{cache_key}', ttl_ms={ttl}, body_size={}): {err}",
                    body.len()
                ),
            )
        }

        let mut response = Response::new(body);
        response.status_code = status_code;
        response.headers = headers;

        Ok(response)
    }
}

fn parse_max_age(header: &[u8]) -> Option<u32> {
    std::str::from_utf8(header)
        .ok()?
        .split(',')
        .find_map(|d| d.trim().strip_prefix("max-age=")?.parse::<u32>().ok())
        .map(|secs| secs * 1000)
}
