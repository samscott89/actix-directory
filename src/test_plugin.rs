use actix::dev::*;
use actix_web::server;
use log::*;
use failure::Error;
use futures::Future;
use serde::{Deserialize, Serialize};
use url::Url;

use std::path::PathBuf;
use std::sync::mpsc;
use std::{time, thread};

use actix_directory::prelude::*;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TestMessage(pub u8);
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TestResponse(pub u8);

impl Message for TestMessage {
	type Result = TestResponse;
}

impl MessageExt for TestMessage {
	const PATH: Option<&'static str> = Some("test");

	type Response = TestResponse;
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TestMessageEmpty;

impl Message for TestMessageEmpty {
	type Result = ();
}

impl MessageExt for TestMessageEmpty {
	const PATH: Option<&'static str> = Some("test_empty");

	type Response = ();
}

#[derive(Default)]
pub struct TestHandler;

impl Actor for TestHandler {
	type Context = Context<Self>;
}

impl Service for TestHandler {
	fn add_to(self, app: app::App) -> app::App {
		let addr = self.start();
		app
		   .route::<TestMessageEmpty, _>(app::no_client(), RouteType::Client)
		   .route::<TestMessage, _>(addr.clone(), RouteType::Server)
		   .route::<TestMessageEmpty, _>(addr.clone(), RouteType::Server)
		   .expose::<TestMessage>()
		   .route(("test", addr.clone()), RouteType::Server)
	}
}

impl Handler<TestMessage> for TestHandler {
	type Result = MessageResult<TestMessage>;

	fn handle(&mut self, msg: TestMessage, _ctxt: &mut Context<Self>) -> Self::Result {
		trace!("Handling TestMessage from TestHandler");
		MessageResult(TestResponse(msg.0))
	}
}


impl Handler<OpaqueMessage> for TestHandler {
	type Result = MessageResult<OpaqueMessage>;

	fn handle(&mut self, msg: OpaqueMessage, _ctxt: &mut Context<Self>) -> Self::Result {
		trace!("Handling TestMessage from TestHandler");
		if msg.id == "test" {
			MessageResult(OpaqueMessage {
				id: "test_response".to_string(),
				inner: b"some reply".to_vec(),
			})
		} else {
			MessageResult(OpaqueMessage {
				id: "err".to_string(),
				inner: b"unknown route".to_vec(),
			})
		}
	}
}

impl Handler<TestMessageEmpty> for TestHandler {
	type Result = ();

	fn handle(&mut self, _msg: TestMessageEmpty, _ctxt: &mut Context<Self>) {
		trace!("Handling TestMessageEmpty from TestHandler");
	}
}

#[derive(Default)]
pub struct TestIntoHandler(pub u8);

impl Actor for TestIntoHandler {
	type Context = Context<Self>;
}

impl Handler<TestMessageEmpty> for TestIntoHandler {
	type Result = FutResponse<TestMessageEmpty>;

	fn handle(&mut self, _msg: TestMessageEmpty, _ctxt: &mut Context<Self>) -> Self::Result {
		trace!("Handling TestMessageEmpty from TestIntoHandler");
		FutResponse(Box::new(app::send(TestMessage(42)).map(|_| ())))
	}
}

use std::env;

#[cfg(unix)]
fn main() {
	init_logger();
	log::info!("Starting plugin");
	let mut args = env::args();
	let _ = args.next();
	let sout = args.next().unwrap();
	let sout = PathBuf::from(sout);
	let sin = args.next().unwrap();
	let sin = PathBuf::from(sin);
	log::info!("Plugin: listening on socket {:?}, server at: {:?}", sin, sout);
	// let mut socket: Option<String> = None;
    let sys = System::new("test_server");
    let addr = TestHandler::default();
    let ad_app = app::App::new()
        .service(addr);

   	let _sock = ad_app.serve_local_http(Some(sin)).clone();
    ad_app.make_current();
    // let server = server::new(app_fact).bind("0.0.0.0:0").unwrap();
    // server.start();
    sys.run();
}

pub fn init_logger() {
	if std::env::var("TEST_LOG").is_ok() {
	    ::std::env::set_var("RUST_LOG", format!("trace,jsonrpc=trace,actix_web={1},actix={0},actix_directory={1}", "trace", "trace"));
	    env_logger::init();
	}
}
