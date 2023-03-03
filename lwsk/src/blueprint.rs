use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{fs, io};

use serde::{Deserialize, Serialize};

use super::KernelConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blueprint {
    functions: HashMap<String, Function>,
    ports: HashMap<String, Port>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Function {
    // The wasm module file
    wasm: PathBuf,

    // Port consumed by this function
    #[serde(default)]
    consumes: Option<String>,

    // Port produced by this function
    #[serde(default)]
    produces: Option<String>,

    // add list of modules it needs to be linked with
    #[serde(default)]
    link_against: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Port {
    // name of the port

    // size in byte of the port
    size: u32,
}

impl Blueprint {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let bp: Blueprint = toml::from_str(&fs::read_to_string(path)?)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        println!("{bp:#?}");
        Ok(bp)
    }

    pub(crate) fn to_kernel_config(&self) -> KernelConfig {
        let mut port_id_map: HashMap<&str, usize> = HashMap::with_capacity(self.ports.len());
        let kernel_ports = self
            .ports
            .iter()
            .enumerate()
            .map(|(idx, (name, bp_port))| {
                port_id_map.insert(name, idx);
                super::KPort {
                    name: name.clone(),
                    buf: vec![0u8; bp_port.size as usize],
                }
            })
            .collect();

        let kernel_functions = self
            .functions
            .iter()
            .map(|(name, bp_func)| {
                let (engine, mut store) = super::initialize_wasm();
                let module =
                    wasmi::Module::new(&engine, fs::File::open(&bp_func.wasm).unwrap()).unwrap();
                let linker = <wasmi::Linker<()>>::new(&engine);
                let instance = linker
                    .instantiate(&mut store, &module)
                    .unwrap()
                    .start(&mut store)
                    .unwrap();

                super::KFunction {
                    name: name.clone(),
                    consumes: bp_func
                        .consumes
                        .as_ref()
                        .map(|name| port_id_map.get(name.as_str()).unwrap().to_owned())
                        .to_owned(),
                    produces: bp_func
                        .produces
                        .as_ref()
                        .map(|name| port_id_map.get(name.as_str()).unwrap().to_owned()),
                    _wasm_mod: module,
                    instance,
                    _engine: engine,
                    store,
                }
            })
            .collect();

        KernelConfig {
            ports: kernel_ports,
            functions: kernel_functions,
        }
    }
}
