use std::collections::HashMap;
use std::path::Path;
use std::{fs, io};

use serde::{Deserialize, Serialize};

use super::KernelConfig;
use crate::Function;

/// Base type of a configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blueprint {
    functions: HashMap<String, EphemeralPartitionBp>,
    channels: HashMap<String, Port>,
    schedules: HashMap<String, Vec<Schedule>>,
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
    size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Schedule {
    Partition { partition: String },
    IoOut { from_port: String, to_io: String },
    IoIn { from_io: String, to_port: String },
    Wait { wait_ns: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Io {
    #[serde(alias = "UDP")]
    Udp { bind: String, connect: String },
}

impl Blueprint {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let bp: Blueprint = toml::from_str(&fs::read_to_string(path)?)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        trace!("parsed blueprint:\n{bp:#?}");
        Ok(bp)
    }

    pub fn to_kernel_config(&self) -> KernelConfig {
        debug!("initializing ports");
        let mut port_id_map: HashMap<&str, usize> = HashMap::with_capacity(self.channels.len());
        let kernel_ports = self
            .channels
            .iter()
            .enumerate()
            .map(|(idx, (name, bp_port))| {
                port_id_map.insert(name, idx);
                trace!("port {name:?} of {} bytes", bp_port.size);
                super::Channel {
                    name: name.clone(),
                    buf: vec![0u8; bp_port.size as usize],
                }
            })
            .collect();

        debug!("loading ephemeral partitions");
        let mut partition_id_map: HashMap<&str, usize> =
            HashMap::with_capacity(self.functions.len());
        let mut kernel_functions = Vec::new();
        for (name, bp_func) in &self.functions {
            let Ok(mut ep) = Function::load(name, &bp_func.wasm) else {
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

            partition_id_map.insert(name, kernel_functions.len() - 1);
        }

        debug!("initializing io drivers");
        let mut io_id_map: HashMap<&str, usize> = HashMap::with_capacity(self.io.len());
        let mut kernel_io: Vec<Box<dyn crate::io::IoDriver>> = Vec::new();
        for (name, io) in &self.io {
            match io {
                Io::Udp { bind, connect } => {
                    let driver = crate::io::udp::Udp::new(bind, connect).unwrap();

                    io_id_map.insert(name, kernel_io.len());
                    kernel_io.push(Box::from(driver));
                }
            };
        }

        debug!("assembling schedules");
        let mut kernel_schedules = Vec::new();
        for (_schedule_name, bp_schedule) in &self.schedules {
            let mut kernel_schedule = Vec::new();
            for slot in bp_schedule {
                kernel_schedule.push(match slot {
                    Schedule::Partition { partition } => {
                        let idx = *partition_id_map.get(partition.as_str()).unwrap();
                        crate::schedule::ScheduleEntry::FunctionInvocation(idx)
                    }
                    Schedule::IoOut { from_port, to_io } => {
                        let from_idx = *port_id_map.get(from_port.as_str()).unwrap();
                        let to_idx = *io_id_map.get(to_io.as_str()).unwrap();
                        crate::schedule::ScheduleEntry::IoOut {
                            from_channel_idx: from_idx,
                            to_io_idx: to_idx,
                        }
                    }
                    Schedule::IoIn { from_io, to_port } => {
                        let from_idx = *io_id_map.get(from_io.as_str()).unwrap();
                        let to_idx = *port_id_map.get(to_port.as_str()).unwrap();
                        crate::schedule::ScheduleEntry::IoOut {
                            from_channel_idx: from_idx,
                            to_io_idx: to_idx,
                        }
                    }
                    Schedule::Wait { wait_ns } => crate::schedule::ScheduleEntry::Wait(
                        core::time::Duration::from_nanos(*wait_ns),
                    ),
                })
            }
            kernel_schedules.push(kernel_schedule);
        }

        debug!("done deriving kernel config");

        KernelConfig {
            channels: kernel_ports,
            functions: kernel_functions,
            schedules: kernel_schedules,
            io: kernel_io,
        }
    }
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
