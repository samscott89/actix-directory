use std::process::Command;

use crate::prelude::*;

impl Plugin {
    pub fn add_to(self, mut app: crate::App) -> crate::App {
        // let socket2 = socket.clone();
        log::trace!("Adding plugin: {:?}", self);
        let Plugin {
            name,
            // version,
            exec_path,
            messages,
            opt_args,
            ty
        } = self;
        let sin = crate::app::sock_path("main");
        // let sin2 = socket.clone();
        let sout = crate::app::sock_path(&name);
        let sout2 = sout.clone();
        // let name2 = name.clone();
        let mut cmd = Command::new(&exec_path);
        if let Some(s) = std::env::var_os("TEST_LOG") {
            cmd.env("TEST_LOG", s);
        }
        let _res = cmd.arg(&sin.to_str().unwrap_or("/dev/null"))
                      .arg(&sout2.to_str().unwrap_or("/dev/null"))
                      .args(opt_args)
                      .spawn().expect("Failed to start plugin");

        for msg in messages.iter() {
            let route = (msg.as_str(), sout.clone());
            app = app.route(route, ty);
        }
        app
    }
}