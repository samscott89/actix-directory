use serde::{Deserialize, Serialize};

use std::any::TypeId;

pub trait Service {
    type Config: Deserialize;

    fn default_client() -> Option<Self> { None }
    fn new(config: Self::Config, app: Addr<app::App>) -> Self;
    fn supported_messages() -> Vec<TypeId>;
}

struct ExternalService;
struct ExternalMessage {
    name: String,
    data: Vec<u8>,
}

impl Service for ExternalService {
    type Config: ();

    fn supported_messages() -> Vec<TypeId> {
        vec![ExternalMessage]
    }
}

fn run<S: Service>(config: String, app: App)-> {
    let config = serde_yaml::from_str(&config).unwrap();
    let service = S::new(config);
    app.add_
}