use ::actix::dev::*;
use failure::Error;
use futures::{future, Future};
use log::*;
// use query_interface::{interfaces, vtable_for};
use serde_derive::{Deserialize, Serialize};

use std::sync::Once;

static START: Once = Once::new();

// use super::*;
use crate::Service;
use crate::service::{IntoHandler, RequestHandler, RespFuture, SoarMessage};

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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TestMessageEmpty;

impl Message for TestMessageEmpty {
	type Result = ();
}

impl SoarMessage for TestMessageEmpty {
	type Response = ();
}


pub struct TestHandler;
// interfaces!(TestHandler: RequestHandler<TestMessage>);

impl RequestHandler<TestMessage> for TestHandler {
	fn handle_request(&self, msg: TestMessage, _service: Addr<Service>) -> RespFuture<TestMessage> {
		trace!("Handling TestMessage from TestHandler");
		Box::new(future::ok(TestResponse(msg.0)))
	}
}

pub struct TestIntoHandlerConfig;
pub struct TestIntoHandler(u8);


impl IntoHandler<TestMessageEmpty> for TestIntoHandlerConfig {
	type Handler = TestIntoHandler;
	fn init(self, service: Addr<Service>) -> Box<Future<Item=Self::Handler, Error=()>> {
		trace!("Creating future for TestIntoHandler");
		Box::new(service.send(TestMessage(12)).map(|resp| {
			trace!("Create TestIntoHandler");
			TestIntoHandler(resp.0)
		}).map_err(|_| ()))
	}
}

impl RequestHandler<TestMessageEmpty> for TestIntoHandler {
	fn handle_request(&self, msg: TestMessageEmpty, service: Addr<Service>) -> RespFuture<TestMessageEmpty> {
		trace!("Handling TestMessageEmpty from TestIntoHandler");
		Box::new(service.send(TestMessage(42)).map(|resp| ()).map_err(Error::from))
	}
}



pub fn init_logger() {
    START.call_once(|| {
	    ::std::env::set_var("RUST_LOG", format!("actix_web={1},actix={0},soar={1}", "trace", "trace"));
	    // ::std::env::set_var("RUST_LOG", "trace");
	    env_logger::init();
    });
}

pub fn start_service<F>(factory: F) -> Addr<Service>
	where F: 'static + Send + Fn() -> Service
{
	Arbiter::start(move |_| factory())
}