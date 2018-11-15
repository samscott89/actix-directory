pub mod http;
pub mod service;

pub use self::service::Service;



#[cfg(test)]
pub mod test_helpers;

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_helpers::*;

	#[test]
	fn test_handler() {
		init_logger();
		let mut sys = actix::System::new("test-sys");
		let mut service = Service::build("test");
		let config = TestIntoHandlerConfig;
		service.add_handler::<TestMessage, _>(TestHandler);
		service.create_handler::<TestMessageEmpty, _>(config);

		let addr = service.start();

		let fut = addr.send(TestMessage(42));
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.0, 42);
		let fut = addr.send(TestMessageEmpty);
		let res = sys.block_on(fut).unwrap();
	}
}
