use actix::Addr;
use actix_web::{http, App, AsyncResponder, HttpMessage, HttpRequest, HttpResponse};
use failure::Error;
use futures::{future, Future};
use log::*;

use crate::MessageExt;
use crate::app;

type AdApp = App<Addr<app::ServerIn>>;
type AppFactory = fn(AdApp, &str) -> AdApp;


/// Used to create a list of functions to apply to a `actix_web::App`
/// in order to properly configure all routes.
#[derive(Clone)]
pub struct HttpFactory
{
    pub factory: Vec<(AppFactory, String)>,
}

fn message<M, H>(app: H, path: &str) -> H
    where
        M: MessageExt,
        H: HttpApp
{
    trace!("Exposing message {:?} on path: {}", crate::get_type!(M), path);
    app.message::<M>(&path)
}

impl Default for HttpFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpFactory {
    pub fn new() -> Self {
        HttpFactory {
            factory: Vec::new(),
        }
    }

    pub fn route<M: MessageExt>(&mut self, path: String) {
        self.factory.push((message::<M, AdApp>, path));
    }

    pub fn configure(&self, app: AdApp) -> AdApp {
        let mut app = app;
        for (f, path) in self.clone().factory {
            app = f(app, &path);
        }
        app
    }
}

/// An `HttpApp` is ultimately used to extend an `actix_web::App`,
/// by adding the method `message`.
///
/// TODO: This should really use content-encoding to differentiate between json/bincode/etc.
pub trait HttpApp {

	/// Register the path `path` as able to respond to requests for the
	/// message type `M`.
	/// Since this will use the `Addr<Service>` in the `App` state,
	/// this handler must have been previously registered.
	fn message<M>(self, path: &str) -> Self
		where
		    M: MessageExt;

    fn jmessage<M>(self, path: &str) -> Self
        where
            M: MessageExt;
}

impl HttpApp for App<Addr<app::ServerIn>> {
	fn message<M>(self, path: &str) -> Self
		where
		    M: MessageExt,
	{
		self.route(path, http::Method::POST, handle_request::<M>)
	}

    fn jmessage<M>(self, path: &str) -> Self
        where
            M: MessageExt,
    {
        self.route(path, http::Method::POST, handle_json_request::<M>)
    }
}

// impl HttpApp for &mut cors::CorsBuilder<Addr<app::ServerIn>> {
//     fn message<M>(self, path: &str) -> Self
//         where
//             M: MessageExt,
//     {
//         self.resource(path, |r| r.post().with(handle_request::<M>))
//     }

//     fn jmessage<M>(self, path: &str) -> Self
//         where
//             M: MessageExt,
//     {
//         self.resource(path, |r| r.post().with(handle_json_request::<M>))
//     }
// }

/// Simple wrapper function. Deserialize request, and serialize the output.
fn handle_json_request<M: 'static>(
    req: HttpRequest<Addr<app::ServerIn>>
) -> impl actix_web::Responder
    where
        M: MessageExt
{
    let addr = req.state().clone();
    req.json().map_err(Error::from)
        .and_then(move |req: M|  {
            trace!("Forwarding message to local handler");
            addr.send(req).map_err(Error::from)
        })
        .map(|resp| {
            trace!("Handled request successfully");
            HttpResponse::Ok().json(resp)
        }).responder()
}

/// Simple wrapper function. Deserialize request, and serialize the output.
fn handle_request<M: 'static>(
    req: HttpRequest<Addr<app::ServerIn>>
) -> impl actix_web::Responder
	where
	    M: MessageExt
{
    let addr = req.state().clone();
    req.body().map_err(Error::from)
    	.and_then(|body| {
    		bincode::deserialize(&body).map_err(|err| {
                error!("Failed to deserialize request: {}", err);
                Error::from(err)
            })
    	})
        .and_then(move |req: M| {
            trace!("Forwarding message to local handler");
            trace!("Actor {:?} is connected: {}", addr, addr.connected());
            addr.send(req).map_err(|err| {
                error!("Failed to send to local handler: {}", err);
                Error::from(err)
            })
        })
        .and_then(|resp| future::result(bincode::serialize(&resp)).map_err(Error::from))
        .map(|resp| {
            trace!("Handled request successfully");
            HttpResponse::Ok().body(resp)
        })
        .responder()
}
