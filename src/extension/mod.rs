// pub struct Extension {
//     pub client_bin: Option<Path>,
//     pub server_bin: Option<Path>,
//     pub messages: Vec<Message>,
// }

// pub struct Message {
//     id: String,
// }

use std::process::{Child, Command, Stdio};

pub struct Handler {
    ctxt: zmq::Context,
}

impl Handler {
    pub fn new() -> Self {
        Self {
            ctxt: zmq::Context::new(),
        }
    }

    pub fn start_plugin_process(
        &self, 
        plugin_desc: PluginDescription,
        // id: PluginId,
        // core: WeakXiCore,
    ) {
        let sockout = self.ctxt.socket(zmq::SocketType::REQ).unwrap()
        let sockin = self.ctxt.socket(zmq::SocketType::REP).unwrap();
        sockout.connect("localhost:*").unwrap();
        sockin.connect("localhost:*").unwrap();
        let out_addr = sockout.get_last_endpoint().unwrap().unwrap();
        let in_addr = sockin.get_last_endpoint().unwrap().unwrap();

        let spawn_result = thread::spawn(move || {
            info!("starting plugin {}", &plugin_desc.name);
            let child = Command::new(&plugin_desc.exec_path)
                .args(&["--out", out_addr])
                .args(&["--in", in_addr])
                .spawn();

            match child {
                Ok(mut child) => {

                    // // set tracing immediately
                    // if xi_trace::is_enabled() {
                    //     plugin.toggle_tracing(true);
                    // }
                    app::plugin_connect(plugin);
                    let err = looper.mainloop(|| BufReader::new(child_stdout), app::rpc_reader());
                    app::plugin_exit(id, err);
                }
                Err(err) => core.plugin_connect(Err(err)),
            }
        });

        if let Err(err) = spawn_result {
            error!("thread spawn failed for {}, {:?}", id, err);
        }
    }

}

type Message = String;

/// Describes attributes and capabilities of a plugin.
///
/// Note: - these will eventually be loaded from manifest files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDescription {
    pub name: String,
    pub version: String,
    // more metadata ...
    /// path to plugin executable
    pub exec_path: PathBuf,
    #[serde(default)]
    pub messages: Vec<Message>,
}
