use actix::Addr;
use actix_web::{http, App, AsyncResponder, HttpMessage, HttpRequest, HttpResponse};
use failure::Error;
use futures::{future, Future};
use log::*;

use crate::{MessageExt, RouteType};
use crate::app;

type AdApp<A> = App<Addr<A>>;
type AppFactory<A> = fn(AdApp<A>, Option<RouteType>) -> AdApp<A>;


/// Used to create a list of functions to apply to a `actix_web::App`
/// in order to properly configure all routes.
#[derive(Default)]
pub struct HttpFactory<A>
    where A: actix::Actor
{
    pub factory: Vec<(AppFactory<A>, Option<RouteType>)>,
}

impl<A> Clone for HttpFactory<A>
    where A: actix::Actor
{
    fn clone(&self) -> Self {
        HttpFactory {
            factory: self.factory.clone()
        }
    }
}

fn message<M, H>(mut app: H, ty: Option<RouteType>) -> H
    where
        M: MessageExt,
        H: HttpApp
{
    let path = format!("/{}", M::PATH);
    trace!("Exposing message {:?} on path: {:?}", crate::get_type!(M), path);
    app = match ty {
        _  => app.message::<M>(&path),
        
    };
    app
    
}

impl<A> HttpFactory<A>
    where A: actix::Actor,
          AdApp<A>: HttpApp
{
    pub fn new() -> Self {
        HttpFactory {
            factory: Vec::new(),
        }
    }

    pub fn route<M: MessageExt>(&mut self, ty: Option<RouteType>) {
        self.factory.push((message::<M, AdApp<A>>, ty));
    }

    pub fn configure(&self, app: AdApp<A>) -> AdApp<A> {
        let mut app = app;
        let f: HttpFactory<A> = self.clone();
        let factory: Vec<(AppFactory<A>, Option<RouteType>)> = f.factory;
        for (f, ty) in factory.into_iter() {
            app = f(app, ty);
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
		self.route(path, http::Method::POST, handle_request::<M, app::ServerIn>)
	}

    fn jmessage<M>(self, path: &str) -> Self
        where
            M: MessageExt,
    {
        self.route(path, http::Method::POST, handle_json_request::<M, app::ServerIn>)
    }
}

impl HttpApp for App<Addr<app::ClientIn>> {
    fn message<M>(self, path: &str) -> Self
        where
            M: MessageExt,
    {
        self.route(path, http::Method::POST, handle_request::<M, app::ClientIn>)
    }

    fn jmessage<M>(self, path: &str) -> Self
        where
            M: MessageExt,
    {
        self.route(path, http::Method::POST, handle_json_request::<M, app::ClientIn>)
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
fn handle_json_request<M, A>(
    req: HttpRequest<Addr<A>>
) -> impl actix_web::Responder
    where
        A: actix::Actor<Context=actix::Context<A>> + actix::Handler<M>,
        M: 'static + MessageExt
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
fn handle_request<M, A>(
    req: HttpRequest<Addr<A>>
) -> impl actix_web::Responder
	where
        A: actix::Actor<Context=actix::Context<A>> + actix::Handler<M>,
	    M: 'static + MessageExt
{
    trace!("Recieved request: {:?}", &req);
    let addr = req.state().clone();
    req.body().map_err(Error::from)
    	.and_then(|body| {
            trace!("Received message: {:?}. Deserialize as {:?}", body, crate::get_type!(M));
    		bincode::deserialize(&body).map_err(|err| {
                error!("Failed to deserialize request: {}", err);
                Error::from(err)
            })
    	})
        .and_then(move |req: M| {
            trace!("Forwarding message to local handler");
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
