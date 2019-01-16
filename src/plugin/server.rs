use actix::dev::*;
use log::*;
use futures::Future;
use serde::{Deserialize, Serialize};

use std::env;
use std::path::PathBuf;

use crate::prelude::*;

pub fn run(app: crate::App) {
	init_logger();
	log::info!("Starting plugin");
	let mut args = env::args();
	let _ = args.next();
	let sockets: Vec<String> = args.collect();
	if sockets.len() != 2 {
		log::error!("Plugin expects two inputs: listening socket and the server socket\n\
					If you are not calling this plugin manually and are seeing this message, please report upstream.");
		std::process::exit(-1i32);
	}
	let (sout, sin) = (&sockets[0], &sockets[1]);
	let sout = PathBuf::from(sout);
	let sin = PathBuf::from(sin);
	log::info!("Plugin: listening on socket {:?}, server at: {:?}", sin, sout);
	// let mut socket: Option<String> = None;
    let sys = System::new("test_server");
   	let _sock = app.serve_local_http(Some(sin)).clone();
    app.make_current();
    sys.run();
}

pub fn init_logger() {
	if std::env::var("TEST_LOG").is_ok() {
	    ::std::env::set_var("RUST_LOG", format!("debug,actix_web={1},actix={1},actix_directory={0}", "trace", "trace"));
	    env_logger::init();
	}
}