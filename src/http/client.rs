//! Create remote actors to use as actors.
// use actix::Addr;
use actix::prelude::*;
use actix_web::{client::ClientRequest, HttpMessage};
use failure::Error;
use futures::{future, Future};
use log::*;
use url::Url;

use std::marker::PhantomData;

use crate::service;
use crate::router::{SoarMessage, SoarResponse};

impl<M: SoarMessage> From<Url> for HttpHandler<M> {
    fn from(other: Url) -> Self {
        HttpHandler(other, PhantomData)
    }
}

/// The `HttpHandler` wraps a `Url` and behaves as a handler for the generic
/// type `M`. This can be registered as a usual `RequestHandler<M>`, but the
/// fact that the actual handler is remote is opaque to the application. 
pub struct HttpHandler<M>(pub Url, PhantomData<M>);

impl<M: 'static> Actor for HttpHandler<M> {
    type Context = Context<Self>;
}

impl<M: SoarMessage> Handler<M> for HttpHandler<M> {
    type Result = SoarResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        let url = self.0.clone();
        let path = url.path().to_string();
        let msg = bincode::serialize(&msg).map_err(Error::from);
        trace!("Channel making request to Actor running at {} on path {}", url.host_str().unwrap_or(""), path);
        let fut = future::result(msg).and_then(move |msg| {
            ClientRequest::post(url)
                .body(msg)
                .unwrap()
                .send()
                .map_err(Error::from)
                .and_then(|resp| {
                    // Deserialize the JSON and map the error
                    resp.body().map_err(Error::from)
                })
                .and_then(|body| {
                    future::result(bincode::deserialize(&body))
                        .map_err(Error::from)
                })
        });
        
        SoarResponse(Box::new(fut))
    }
}

#[cfg(test)]
mod tests {
    use actix::Actor;
    use actix_web::test::TestServer;

    use super::*;
    use crate::test_helpers::*;
    use crate::router;

    #[test]
    fn test_http_channel() {
        init_logger();
        let mut sys = System::new("test");
        let mut server = TestServer::new(|app| {
            app.resource("/test", |r| r.f(|_| {
                trace!("Received request! Responding with answer");
                let msg = bincode::serialize(&TestResponse(138)).unwrap();
                actix_web::HttpResponse::Ok().body(msg)
            }));
        });

        let url = Url::parse(&server.url("/test")).unwrap();
        trace!("Test URL: {:?}", url);

        let handler = HttpHandler::from(url).start();
        router::add_route::<TestMessage, _>(handler);
        let res = sys.block_on(router::send(TestMessage(138))).unwrap();
        assert_eq!(res.0, 138);
    }
}