//! Create remote actors to use as actors.
use actix::Addr;
use actix_web::{client::ClientRequest, HttpMessage};
use failure::Error;
use futures::Future;
use log::*;
use query_interface::{interfaces, vtable_for};
use url::Url;

use std::marker::PhantomData;

use crate::service::*;

impl<M: SoarMessage> From<Url> for HttpHandler<M> {
    fn from(other: Url) -> Self {
        HttpHandler(other, PhantomData)
    }
}

/// The `HttpHandler` wraps a `Url` and behaves as a handler for the generic
/// type `M`. This can be registered as a usual `RequestHandler<M>`, but the
/// fact that the actual handler is remote is opaque to the application. 
pub struct HttpHandler<M>(pub Url, PhantomData<M>);
interfaces!(<M: SoarMessage> HttpHandler<M>: RequestHandler<M>);

impl<M: SoarMessage> RequestHandler<M> for HttpHandler<M> {
    fn handle_request(&self, msg: M, _: Addr<Service>) -> RespFuture<M> {
        let url = self.0.clone();
        let path = url.path().to_string();
        trace!("Channel making request to Actor running at {} on path {}", url.host_str().unwrap_or(""), path);
        Box::new(ClientRequest::post(url)
            .json(msg)
            .unwrap()
            .send()
            .map_err(Error::from)
            .and_then(|resp| {
                // Deserialize the JSON and map the error
                resp.json().map_err(Error::from)
            }))
    }
}

#[cfg(test)]
mod tests {
    use actix_web::test::TestServer;

    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn test_http_channel() {
        init_logger();
        let mut server = TestServer::new(|app| {
            app.resource("/test", |r| r.f(|_| {
                trace!("Received request! Responding with answer");
                actix_web::HttpResponse::Ok().json(TestResponse(138))
            }));
        });

        let url = Url::parse(&server.url("/test")).unwrap();
        trace!("Test URL: {:?}", url);
        let res = server.execute(futures::future::lazy(|| {
            let addr = Service::build("http_channel_test_client")
                                        .add_http_handler::<TestMessage>(url.clone())
                                        .address();
            addr.send(TestMessage(138))
        })).unwrap();
        assert_eq!(res.0, 138);
    }
}