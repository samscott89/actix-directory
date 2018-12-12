#![cfg_attr(feature="print_types", feature(core_intrinsics))]

pub mod http;
pub mod service;
pub(crate) mod router;

pub use self::router::{SoarMessage, SoarResponse};

#[cfg(test)]
pub mod test_helpers;

#[cfg(test)]
mod tests {
	use actix::Actor;
	use futures::future;

	use super::*;

	use crate::router;
	use crate::service;
	use crate::test_helpers::*;

	#[test]
	fn test_service() {
		init_logger();
		let mut sys = actix::System::new("test-sys");

		let handler = TestHandler::start_default();
		let service = service::Service::new()
			.add_service::<TestMessage, _, _>(service::no_client(), handler)
			.run();

		let fut = service::send(TestMessage(42));
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.0, 42);
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

