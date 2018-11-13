use ::actix::dev::*;
use futures::future;
use query_interface::{interfaces, vtable_for};
use serde_derive::{Deserialize, Serialize};

use std::sync::Once;

static START: Once = Once::new();

use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TestMessage(pub u8);
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TestResponse(pub u8);

impl Message for TestMessage {
	type Result = TestResponse;
}

impl SoarMessage for TestMessage {
	type Response = TestResponse;
}

pub struct TestHandler;
interfaces!(TestHandler: RequestHandler<TestMessage>);

impl RequestHandler<TestMessage> for TestHandler {
	fn handle_request(&self, msg: TestMessage, _service: &Service) -> Box<Future<Item=TestResponse, Error=Error>> {
		Box::new(future::ok(TestResponse(msg.0)))
	}
}


pub fn init_logger() {
    START.call_once(|| {
	    // ::std::env::set_var("RUST_LOG", format!("actix_web={1},actix={0},soar={1}", "trace", "trace"));
	    // ::std::env::set_var("RUST_LOG", "trace");
	    env_logger::init();
    });
}

pub fn start_service<F>(factory: F) -> Addr<Service>
	where F: 'static + Send + Fn() -> Service
{
	Arbiter::start(move |_| factory())
}