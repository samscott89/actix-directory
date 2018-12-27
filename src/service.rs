use actix::dev::*;
use crate::app::App;

pub trait Service: Sized {
    fn add_to(self, app: &mut App) -> &mut App { app }
}
