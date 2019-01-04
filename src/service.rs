//! Group together routes into a single `Service`.

use actix::dev::*;
use crate::app::App;
use crate::*;

/// Group together functionality into a service.
/// Makes it easier to bulk add routes to an `App`.
pub trait Service: Sized {
    fn add_to(self, app: App) -> App { app }
}

macro_rules! service_impl {
	($name:ident, $($service:ident),+) => {
		#[allow(non_snake_case)]
		pub struct $name<$($service,)*>
			where $($service: MessageExt,)*
		{
			$(
			pub $service: Recipient<$service>,
			)*
		}

		impl<$($service,)*> Service for $name<$($service,)*>
			where $($service: MessageExt,)*
		{
			fn add_to(self, app: App) -> App {
				app
					$(.route::<$service, _>(self.$service, RouteType::Server))*
			}
		}

	}
}

service_impl!(S1, M1);
service_impl!(S2, M1, M2);
service_impl!(S3, M1, M2, M3);
