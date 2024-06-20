use std::collections::HashMap;
use std::path::Path;
use std::{fs, io};

use serde::{Deserialize, Serialize};

use super::KernelConfig;
use crate::Function;

/// Base type of a configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blueprint {
    functions: HashMap<String, FunctionBp>,
    channels: HashMap<String, ChannelBp>,
    schedules: HashMap<String, Vec<ScheduleBp>>,
    io: HashMap<String, IoBp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunctionBp {
    // The WASM module file
    wasm: String,

    // Channel consumed by this function
    #[serde(default)]
    consumes: Option<String>,

    // Channel produced by this function
    #[serde(default)]
    produces: Option<String>,

    /// Amount of fuel to provide per call
    fuel_per_call: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelBp {
    /// Size in byte of the channel
    size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScheduleBp {
    Function { function: String },
    IoOut { from_channel: String, to_io: String },
    IoIn { from_io: String, to_channel: String },
    Wait { wait_ns: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IoBp {
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
        debug!("initializing channels");
        let mut channel_id_map: HashMap<&str, usize> = HashMap::with_capacity(self.channels.len());
        let kernel_channels = self
            .channels
            .iter()
            .enumerate()
            .map(|(idx, (name, bp_channel))| {
                channel_id_map.insert(name, idx);
                trace!("{name:?}/channel[{idx}] size is {} bytes", bp_channel.size);
                super::Channel {
                    name: name.clone(),
                    buf: vec![0u8; bp_channel.size],
                }
            })
            .collect();

        debug!("loading Wasm function");
        let mut function_id_map: HashMap<&str, usize> =
            HashMap::with_capacity(self.functions.len());
        let mut kernel_functions = Vec::new();
        for (name, bp_func) in &self.functions {
            let Ok(mut f) = Function::load(name, &bp_func.wasm) else {
                warn!("error during initialization, skipping");
                continue;
            };

            bp_func
                .consumes
                .as_ref()
                .map(|name| channel_id_map.get(name.as_str()).unwrap().to_owned())
                .clone_into(&mut f.consumes);

            f.produces = bp_func
                .produces
                .as_ref()
                .map(|name| channel_id_map.get(name.as_str()).unwrap().to_owned());

            f.fuel_per_call = bp_func.fuel_per_call;

            kernel_functions.push(f);

            function_id_map.insert(name, kernel_functions.len() - 1);
        }

        debug!("initializing io drivers");
        let mut io_id_map: HashMap<&str, usize> = HashMap::with_capacity(self.io.len());
        let mut kernel_io: Vec<Box<dyn crate::io::IoDriver>> = Vec::new();
        for (name, io) in &self.io {
            match io {
                IoBp::Udp { bind, connect } => {
                    let driver = crate::io::udp::Udp::new(bind, connect).unwrap();

                    io_id_map.insert(name, kernel_io.len());
                    kernel_io.push(Box::from(driver));
                }
            };
        }

        debug!("assembling schedules");
        let mut kernel_schedules = Vec::new();
        for bp_schedule in self.schedules.values() {
            let mut kernel_schedule = Vec::new();
            for slot in bp_schedule {
                kernel_schedule.push(match slot {
                    ScheduleBp::Function { function } => {
                        let idx = *function_id_map.get(function.as_str()).unwrap();
                        crate::schedule::ScheduleEntry::FunctionInvocation(idx)
                    }
                    ScheduleBp::IoOut {
                        from_channel,
                        to_io,
                    } => {
                        let from_idx = *channel_id_map.get(from_channel.as_str()).unwrap();
                        let to_idx = *io_id_map.get(to_io.as_str()).unwrap();
                        crate::schedule::ScheduleEntry::IoOut {
                            from_channel_idx: from_idx,
                            to_io_idx: to_idx,
                        }
                    }
                    ScheduleBp::IoIn {
                        from_io,
                        to_channel,
                    } => {
                        let from_idx = *io_id_map.get(from_io.as_str()).unwrap();
                        let to_idx = *channel_id_map.get(to_channel.as_str()).unwrap();
                        crate::schedule::ScheduleEntry::IoOut {
                            from_channel_idx: from_idx,
                            to_io_idx: to_idx,
                        }
                    }
                    ScheduleBp::Wait { wait_ns } => crate::schedule::ScheduleEntry::Wait(
                        core::time::Duration::from_nanos(*wait_ns),
                    ),
                })
            }
            kernel_schedules.push(kernel_schedule);
        }

        debug!("done deriving kernel config");

        KernelConfig {
            channels: kernel_channels,
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
