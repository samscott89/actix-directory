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
		let config = TestIntoHandlerConfig;
		let mut sys = actix::System::new("test-sys");

		let addr = Service::build("test")
						.add_handler::<TestMessage, _>(TestHandler)
						.create_handler::<TestMessageEmpty, _>(config)
						.address();

		let fut = addr.send(TestMessage(42));
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.0, 42);
		let fut = addr.send(TestMessageEmpty);
		let res = sys.block_on(fut).unwrap();
	}
}
