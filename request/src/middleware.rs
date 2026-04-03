use crate::{RawRequest, Response, error::Error};

pub trait Middleware {
    fn handle(&self, req: RawRequest, next: Next<'_>) -> Result<Response, Error>;
}

pub struct Next<'a> {
    middlewares: &'a [Box<dyn Middleware + 'a>],
}

impl<'a> Next<'a> {
    pub(crate) fn new(middlewares: &'a [Box<dyn Middleware + 'a>]) -> Self {
        Self { middlewares }
    }

    pub fn run(&self, req: RawRequest) -> Result<Response, Error> {
        match self.middlewares.split_first() {
            Some((head, tail)) => head.handle(req, Next { middlewares: tail }),
            None => req.send(),
        }
    }
}
