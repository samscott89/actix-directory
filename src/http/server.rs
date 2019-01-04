use actix_web::{http, App, AsyncResponder, HttpMessage, HttpRequest, HttpResponse};
use actix_web::middleware::cors;
use failure::Error;
use futures::{future, Future};
use log::*;

use crate::MessageExt;
use crate::app;

pub(crate) struct HttpFactory {
    pub factory: Box<Fn(App) -> App>,
    pub factory_cors: Box<Fn(&mut cors::CorsBuilder) -> &mut cors::CorsBuilder>,
    #[cfg(test)]
    pub factory_test: Box<Fn(&mut actix_web::test::TestApp) -> &mut actix_web::test::TestApp>,

}

impl Default for HttpFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpFactory {
    pub fn new() -> Self {
        HttpFactory {
            factory: Box::new(|app| app),
            factory_cors: Box::new(|app| app),
            #[cfg(test)]
            factory_test: Box::new(|app| app),
        }
    }

    pub fn route<M: MessageExt>(self, path: String) -> Self {
        let HttpFactory {
            factory, factory_cors, 
            #[cfg(test)]
            factory_test,
        } = self;
        let path2 = path.clone();
        #[cfg(test)]
        let path3 = path.clone();
        HttpFactory {
            factory: Box::new(move |app| { (factory)(app).message::<M>(&path) }),
            factory_cors: Box::new(move |app| { (factory_cors)(app).message::<M>(&path2) }),
            #[cfg(test)]
            factory_test: Box::new(move |app| { (factory_test)(app).message::<M>(&path3) }),
        }
    }

    pub fn configure(&self, app: App) -> App {
        (self.factory)(app)
    }

    pub fn configure_cors<'a>(&self, app: &'a mut cors::CorsBuilder) -> &'a mut cors::CorsBuilder {
        (self.factory_cors)(app)
    }

    #[cfg(test)]
    pub fn configure_test<'a>(&self, app: &'a mut actix_web::test::TestApp) -> &'a mut actix_web::test::TestApp {
        (self.factory_test)(app)
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

impl HttpApp for App {
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

impl HttpApp for &mut cors::CorsBuilder {
    fn message<M>(self, path: &str) -> Self
        where
            M: MessageExt,
    {
        self.resource(path, |r| r.post().with(handle_request::<M>))
    }

    fn jmessage<M>(self, path: &str) -> Self
        where
            M: MessageExt,
    {
        self.resource(path, |r| r.post().with(handle_json_request::<M>))
    }

}
#[cfg(test)]
impl HttpApp for &mut actix_web::test::TestApp {
	fn message<M>(self, path: &str) -> Self
		where
		    M: MessageExt
	{
		trace!("TEST: Handle message");
        self.resource(path, |r| r.post().with(handle_request::<M>))
	}

    fn jmessage<M>(self, path: &str) -> Self
        where
            M: MessageExt
    {
        trace!("TEST: Handle message");
        self.resource(path, |r| r.post().with(handle_json_request::<M>))
    }
}


/// Simple wrapper function. Deserialize request, and serialize the output.
fn handle_json_request<M: 'static>(
    req: HttpRequest,
) -> impl actix_web::Responder
    where
        M: MessageExt
{
    req.json().map_err(Error::from)
        .and_then(move |req: M|  {
            trace!("Forwarding message to local handler");
            app::send_in(req).map_err(Error::from)
        })
        .map(|resp| {
            trace!("Handled request successfully");
            HttpResponse::Ok().json(resp)
        }).responder()
}

/// Simple wrapper function. Deserialize request, and serialize the output.
fn handle_request<M: 'static>(
    req: HttpRequest,
) -> impl actix_web::Responder
	where
	    M: MessageExt
{
    req.body().map_err(Error::from)
    	.and_then(|body| {
    		bincode::deserialize(&body).map_err(Error::from)
    	})
        .and_then(move |req: M| {
            trace!("Forwarding message to local handler");
            app::send_in(req).map_err(Error::from)
        })
        .and_then(|resp| future::result(bincode::serialize(&resp)).map_err(Error::from))
        .map(|resp| {
            trace!("Handled request successfully");
            HttpResponse::Ok().body(resp)
        })
        .responder()
}
