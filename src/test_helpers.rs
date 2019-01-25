use ::actix::dev::*;
use futures::Future;
use log::*;
use serde::{Deserialize, Serialize};

use std::path::PathBuf;
use std::sync::Once;

static START: Once = Once::new();

use crate::prelude::*;

pub fn test_plugin() -> Plugin {
	Plugin {
		name: "test_plugin".to_string(),
		// version: "0.1"
		exec_path: PathBuf::from("./target/debug/test-plugin"),
		messages: vec![
			"test".to_string(),
			"test_empty".to_string(),
		],
		opt_args: Vec::new(),
	}
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TestMessage(pub u8);
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TestResponse(pub u8);

impl Message for TestMessage {
	type Result = TestResponse;
}

impl MessageExt for TestMessage {
	const PATH: &'static str = "test";

	type Response = TestResponse;
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TestMessageEmpty;

impl Message for TestMessageEmpty {
	type Result = ();
}

impl MessageExt for TestMessageEmpty {
	const PATH: &'static str = "test_empty";

	type Response = ();
}

#[derive(Default)]
pub struct TestHandler;

impl Actor for TestHandler {
	type Context = Context<Self>;
}

impl Service for TestHandler {
	fn add_to(self, app: App) -> App {
		let addr = self.start();
		app
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

pub fn init_logger() {
    START.call_once(|| {
    	if std::env::var("TEST_LOG").is_ok() {
		    ::std::env::set_var("RUST_LOG", format!("debug,actix_web={1},actix={1},actix_directory={0}", "trace", "trace"));
		    env_logger::init();
    	}
    });
}
