use ::actix::dev::*;
use futures::future;
use query_interface::{interfaces, vtable_for};
use serde_derive::{Deserialize, Serialize};

use std::env;
use std::sync::mpsc;
use std::thread;

use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TestMessage(pub u8);
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TestResponse(pub u8);

impl Message for TestMessage {
	type Result = Result<TestResponse, ()>;
}

pub struct TestHandler;
interfaces!(TestHandler: RequestHandler<TestMessage, TestResponse, ()>);

impl RequestHandler<TestMessage, TestResponse, ()> for TestHandler {
	fn handle_request(&self, msg: TestMessage, service: &Service) -> ResponseFuture<TestResponse, ()> {
		Box::new(future::ok(TestResponse(msg.0)))
	}
}

pub fn init_logger() {
    env::set_var("RUST_LOG", format!("actix_web={1},actix={0},soar={1}", "trace", "trace"));
    env_logger::init();
}

pub fn start_service<F>(factory: F) -> Addr<Service>
	where F: 'static + Send + Fn() -> Service
{
	// let (tx, rx) = mpsc::channel();

	// thread::spawn(move || {
	// 	let sys = System::new("soar-test-server");
	// 	let addr = factory().start();
	// 	tx.send((System::current(), addr)).unwrap();
	// });

	// let (sys, addr) = rx.recv().unwrap();
	// System::set_current(sys);
	// addr
	Arbiter::start(move |_| factory())
}