use crate::config::{Config, DatabaseCredential};
use std::path::PathBuf;
// Fixed types select a logical identity; an HTTP parameter cannot select one.
// Every adapter acquisition reloads the atomic file and refuses expiry.
#[derive(Debug)]
pub struct Workspace {
    path: PathBuf,
}
#[derive(Debug)]
pub struct Control {
    path: PathBuf,
}
#[derive(Debug)]
pub struct Data {
    path: PathBuf,
}
#[derive(Debug)]
pub struct Worker {
    path: PathBuf,
}
#[derive(Debug)]
pub struct Services {
    pub workspace: Workspace,
    pub control: Control,
    pub data: Data,
    pub worker: Worker,
}
impl Services {
    pub fn new(path: PathBuf) -> Self {
        Self {
            workspace: Workspace { path: path.clone() },
            control: Control { path: path.clone() },
            data: Data { path: path.clone() },
            worker: Worker { path },
        }
    }
}
macro_rules! adapter {
    ($role:ty, $field:ident) => {
        impl $role {
            pub fn database_credential(&self) -> Result<DatabaseCredential, &'static str> {
                Ok(Config::load(&self.path)?.$field)
            }
        }
    };
}
adapter!(Workspace, workspace);
adapter!(Control, control);
adapter!(Data, data);
adapter!(Worker, worker);
