#![cfg_attr(feature="print_types", feature(core_intrinsics))]

pub mod http;
pub mod service;

#[cfg(test)]
pub mod test_helpers;

#[cfg(test)]
mod tests {
	use actix::Actor;
	use futures::future;

	use super::*;

	use crate::service;
	use crate::test_helpers::*;

	#[test]
	fn test_handler() {
		init_logger();
		let mut sys = actix::System::new("test-sys");

		let handler = TestHandler::start_default();
		service::add_route::<TestMessage, _>(handler);
		let handler_fut = future::ok(TestIntoHandler(12).start());
		service::add_route_fut(handler_fut);

		let fut = service::send(TestMessage(42));
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.0, 42);
		let fut = service::send(TestMessageEmpty);
		let _res = sys.block_on(fut).unwrap();
	}
}

#[cfg(not(feature = "print_types"))]
#[macro_export]
macro_rules! get_type {
	($T:ty) => (
		::std::any::TypeId::of::<$T>()
	)
}
#[cfg(feature = "print_types")]
#[macro_export]
macro_rules! get_type {
	($T:ty) => (
		unsafe { ::std::intrinsics::type_name::<$T>() }
	)
}

