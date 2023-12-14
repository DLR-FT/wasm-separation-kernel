use std::collections::HashMap;
use std::path::Path;
use std::{fs, io};

use serde::{Deserialize, Serialize};

use super::KernelConfig;
use crate::{drivers, EphemeralPartition};

/// Base type of a configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blueprint {
    partitions: HashMap<String, EphemeralPartitionBp>,
    ports: HashMap<String, Port>,
    // schedules: Vec<Schedule>,
    io: HashMap<String, Io>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EphemeralPartitionBp {
    // The WASM module file
    wasm: String,

    // Port consumed by this function
    #[serde(default)]
    consumes: Option<String>,

    // Port produced by this function
    #[serde(default)]
    produces: Option<String>,

    /// Amount of fuel to provide per call
    fuel_per_call: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Port {
    // size in byte of the port
    size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Io {
    #[serde(alias = "UDP")]
    Udp {
        port: u16,
        // size: u32,
        produces: String,
        min_interval_ns: u64,
    },
}

impl Blueprint {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let bp: Blueprint = toml::from_str(&fs::read_to_string(path)?)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        println!("{bp:#?}");
        Ok(bp)
    }

    pub fn to_kernel_config(&self) -> KernelConfig {
        let mut port_id_map: HashMap<&str, usize> = HashMap::with_capacity(self.ports.len());

        debug!("initialializing ports");
        let kernel_ports = self
            .ports
            .iter()
            .enumerate()
            .map(|(idx, (name, bp_port))| {
                port_id_map.insert(name, idx);
                trace!("port {name:?} of {} bytes", bp_port.size);
                super::KPort {
                    name: name.clone(),
                    buf: vec![0u8; bp_port.size as usize],
                }
            })
            .collect();

        debug!("loading ephemeral partitions");
        let mut kernel_functions = Vec::new();
        for (name, bp_func) in &self.partitions {
            let Ok(mut ep) = EphemeralPartition::load(name, &bp_func.wasm) else {
                warn!("error during initialization, skipping");
                continue;
            };

            ep.consumes = bp_func
                .consumes
                .as_ref()
                .map(|name| port_id_map.get(name.as_str()).unwrap().to_owned())
                .to_owned();

            ep.produces = bp_func
                .produces
                .as_ref()
                .map(|name| port_id_map.get(name.as_str()).unwrap().to_owned());

            kernel_functions.push(ep);
        }

        for (name, io) in &self.io {
            match io {
                Io::Udp { port, produces, .. } => drivers::UdpDriver::new(*port),
            };
        }

        KernelConfig {
            ports: kernel_ports,
            functions: kernel_functions,
        }
    }
}

pub struct Schedule {
    offset: u64,
    triggers: String,
}

/// What to do with the linear memory of an interpreter when a timeout occured
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OnTimeAbort {
    /// Reset the linear memory to the initial state after loading the WASM
    Reset,

    /// Reset the linear memory to the value prior to the last function call in this interpreter
    LastCheckPoint,

    /// Keep the linear memory exactly as is.
    /// Warning: this is dangerous, you must ensure that all state is checked before usage
    Keep,
}
