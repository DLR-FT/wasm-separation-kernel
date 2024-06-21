use core::time::Duration;
use std::io::Read;

use wasmi::TypedFunc;

use crate::schedule::{Schedule, ScheduleEntry};
use crate::LwskError;

pub const ENTRY_FUNCTION_NAME: &str = "process";

pub struct KernelConfig {
    /// Communication channels which can be read from or written to by either functions or IO
    /// drivers
    pub channels: Vec<Channel>,

    /// Available functions and their state
    pub functions: Vec<Function>,

    /// There can be multiple schedules, hence this is a [Vec] of [Vec]
    pub schedules: Vec<Schedule>,

    /// IO driver which allow to connect external information sources and sinks to channels
    pub io: Vec<Box<dyn crate::io::IoDriver>>,

    /// Index of the initial schedule
    pub current_schedule_idx: usize,
}

pub struct KernelState {}

/// A function as defined in the servereless idiom
///
/// Functions are characterized by having an actual entry function (as in a callable Wasm function),
/// which is executed with a configured input data.
pub struct Function {
    /// Name of this function
    pub name: String,

    /// Index of the channel that this function consumes upon invocation, if any
    pub consumes: Option<usize>,

    /// Index of the channel that this function provides data to when terminating, if any
    pub produces: Option<usize>,

    /// Parsed Wasm of this [Function]
    pub _wasm_mod: wasmi::Module,

    /// Wasm engine of this [Function]
    pub _engine: wasmi::Engine,

    /// Wasm store of this [Function]
    pub store: wasmi::Store<()>,

    /// Wasm instance of this [Function]
    pub instance: wasmi::Instance,

    /// Upper limit of fuel available per call to this function
    pub fuel_per_call: u64,
}

/// A place in memory to hold state
///
/// Channels allow information/state to be passed between Functions, or to/from IO drivers
pub struct Channel {
    /// Name of this channel
    pub name: String,

    /// Buffer backing up the data
    pub buf: Vec<u8>,
}

impl KernelConfig {
    /// Checks that the [KernelConfig] is valid
    ///
    /// # Checks
    ///
    /// - for each [Function], that ...
    ///   - ... the index in consumes points to an existing channel, if any
    ///   - ... the index in produeces points to an existing channel, if any
    /// - for each [ScheduleEntry], that ...
    ///   - ... it references an existing function, if any
    ///   - ... it references an existing channel, if any
    ///   - ... it references an existing io, if any
    pub fn validate(&self) -> Result<(), LwskError> {
        for (function_idx, f) in self.functions.iter().enumerate() {
            if let Some(channel_idx) = f.consumes {
                debug!(
                    "checking existance of {:?}/functions[{function_idx}].consumes AKA channels[{channel_idx}]",
                    f.name
                );
                if self.channels.get(channel_idx).is_none() {
                    error!("channels[{channel_idx}] does not exist");
                    return Err(LwskError::InvalidChannelIdx(channel_idx));
                }

                let global_name = "INPUT";
                debug!(
                    "checking existance of {:?}/functions[{function_idx}] {global_name:?} global",
                    f.name
                );
                if f.get_global(global_name, self.channels[channel_idx].buf.len())
                    .is_err()
                {
                    error!("{:?}/functions[{function_idx}] {global_name:?} global does not exist or is of wrong size", f.name);
                    return Err(LwskError::WasmLoadError);
                }
            }
            if let Some(channel_idx) = f.produces {
                debug!(
                    "checking existance of {:?}/functions[{function_idx}].produces AKA channels[{channel_idx}]",
                    f.name
                );
                if self.channels.get(channel_idx).is_none() {
                    error!("channels[{channel_idx}] does not exist");
                    return Err(LwskError::InvalidChannelIdx(channel_idx));
                }

                let global_name = "OUTPUT";
                debug!(
                    "checking existance of {:?}/functions[{function_idx}] {global_name:?} global",
                    f.name
                );
                if f.get_global(global_name, self.channels[channel_idx].buf.len())
                    .is_err()
                {
                    error!("{:?}/functions[{function_idx}] {global_name:?} global does not exist or is of wrong size", f.name);
                    return Err(LwskError::WasmLoadError);
                }
            }

            debug!(
                    "checking existance of {:?}/functions[{function_idx}] entry function {ENTRY_FUNCTION_NAME:?}", f.name
                    );
            if f.get_entry_function().is_err() {
                error!(
                    "{:?}/functions[{function_idx}] has no valid entry function",
                    f.name
                );
                return Err(LwskError::WasmLoadError);
            }
        }

        for sched in &self.schedules {
            for entry in &sched.sequence {
                match entry {
                    ScheduleEntry::FunctionInvocation(function_idx) => {
                        // can not contain name of function, as we don't know function to exist
                        debug!("checking existance of functions[{function_idx}]");
                        if self.functions.get(*function_idx).is_none() {
                            error!("functions[{function_idx}] does not exist");
                            return Err(LwskError::InvalidFunctionIdx(*function_idx));
                        }
                    }
                    ScheduleEntry::IoIn {
                        from_io_idx,
                        to_channel_idx,
                    } => {
                        debug!("checking from io[{from_io_idx}] to channels[{to_channel_idx}] is possible");

                        if self.io.get(*from_io_idx).is_none() {
                            error!("io[{from_io_idx}] does not exist");
                            return Err(LwskError::InvalidIoIdx(*from_io_idx));
                        }

                        if self.channels.get(*to_channel_idx).is_none() {
                            error!("channels[{to_channel_idx}] does not exist");
                            return Err(LwskError::InvalidChannelIdx(*to_channel_idx));
                        }
                    }
                    ScheduleEntry::IoOut {
                        from_channel_idx,
                        to_io_idx,
                    } => {
                        debug!("checking from channel[{from_channel_idx}] to io[{to_io_idx}] is possible");

                        if self.channels.get(*from_channel_idx).is_none() {
                            error!("channel[{from_channel_idx}] does not exist");
                            return Err(LwskError::InvalidChannelIdx(*from_channel_idx));
                        }

                        if self.io.get(*to_io_idx).is_none() {
                            error!("io[{to_io_idx}] does not exist");
                            return Err(LwskError::InvalidIoIdx(*to_io_idx));
                        }
                    }
                    ScheduleEntry::Wait(duration) => {
                        if *duration > Duration::from_secs(10) {
                            warn!("found a duration greater than 10 s, that might hurt real-time performance bad");
                        }
                    }
                    ScheduleEntry::SwitchSchedule(schedule_idx) => {
                        debug!("checking existance of schedules[{schedule_idx}]",);
                        if sched.sequence.get(*schedule_idx).is_none() {
                            error!("schedules[{schedule_idx}] does not exist");
                            return Err(LwskError::WasmLoadError);
                        }
                    }
                }
            }
        }
        info!("validation complete, no errors");

        Ok(())
    }
}

pub fn initialize_wasm() -> (wasmi::Engine, wasmi::Store<()>) {
    let mut config = wasmi::Config::default();
    config.consume_fuel(true);
    let engine = wasmi::Engine::new(&config);
    let store = wasmi::Store::new(&engine, ());
    (engine, store)
}

type EntryFunctionType = TypedFunc<(), i32>;

impl Function {
    pub fn load(name: &str, wasm_module_path: &str) -> Result<Self, LwskError> {
        trace!("loading function {name:?} from {wasm_module_path:?}");

        #[cfg(feature = "std")]
        let wasm_bytes = {
            let mut buf = Vec::new();
            if let Err(e) =
                std::fs::File::open(wasm_module_path).and_then(|mut f| f.read_to_end(&mut buf))
            {
                error!("could not open file {wasm_module_path:?}: {e}");
                return Err(LwskError::WasmLoadError);
            }
            buf
        };

        #[cfg(not(feature = "std"))]
        let wasm_bytes = {
            todo!("interprete the path as resource descriptor for a fit image or something, get a byte slice, be done with it")
        };

        let (engine, mut store) = super::initialize_wasm();

        trace!("parsing wasm file");
        let module = match wasmi::Module::new(&engine, wasm_bytes.as_slice()) {
            Ok(module) => module,
            Err(e) => {
                error!("could not load wasm module: {e}");
                return Err(LwskError::WasmLoadError);
            }
        };

        trace!("linking wasm module");
        let linker = <wasmi::Linker<()>>::new(&engine);
        let instance = match linker.instantiate(&mut store, &module) {
            Ok(instance) => instance,
            Err(e) => {
                error!("could not link wasm module {wasm_module_path:?}: {e}");
                return Err(LwskError::WasmLoadError);
            }
        };

        trace!("starting wasm module");
        // TODO verify that this is real-time compatible
        let started_instance = match instance.start(&mut store) {
            Ok(instance) => instance,
            Err(e) => {
                error!("could not start wasm module {wasm_module_path:?}: {e}");
                return Err(LwskError::WasmLoadError);
            }
        };

        Ok(Self {
            name: name.into(),
            consumes: None,
            produces: None,
            _wasm_mod: module,
            _engine: engine,
            store,
            instance: started_instance,
            fuel_per_call: 0,
        })
    }

    pub fn get_entry_function(&self) -> Result<EntryFunctionType, LwskError> {
        use wasmi::errors::*;
        self.instance
            .get_typed_func::<(), i32>(&self.store, ENTRY_FUNCTION_NAME)
            .map_err(|e| match e.kind() {
                ErrorKind::Func(FuncError::ExportedFuncNotFound) => {
                    error!(
                        "wasm function {ENTRY_FUNCTION_NAME:?} not found in {:?}",
                        self.name
                    );
                    LwskError::WasmLoadError
                }
                ErrorKind::Func(FuncError::MismatchingParameterLen) => {
                    error!(
                        "wasm function {ENTRY_FUNCTION_NAME:?} of {:?} has mismatching type signature",
                        self.name
                    );
                    LwskError::WasmLoadError
                }
                e => {
                    error!("{e}");
                    LwskError::WasmLoadError
                }
            })
    }

    /// Get the index of a global inside this function's wasm module
    pub fn get_global_idx(&self, ident: &str) -> Result<i32, LwskError> {
        self.instance
            .get_global(&self.store, ident)
            .ok_or(LwskError::GlobalDoesNotExist)
            .map_err(|x| {
                error!(
                    "could not find global {ident:?} in wasm module {:?}",
                    self.name
                );
                x
            })?
            .get(&self.store)
            .i32()
            .ok_or(LwskError::UnexpectedWasmType)
            .map_err(|x| {
                error!("global {ident:?} is not of type i32");
                x
            })
    }

    /// Get a shared ref to the memory backing a global in this functions Wasm module
    pub fn get_global(&self, ident: &str, len: usize) -> Result<&[u8], LwskError> {
        let idx = self.get_global_idx(ident)? as usize;

        let mem_name = "memory";
        let memory = self
            .instance
            .get_memory(&self.store, mem_name)
            .ok_or(LwskError::NoSuchWasmMemory)
            .map_err(|x| {
                error!(
                    "no memory named {mem_name:?} was found in wasm module {:?}",
                    self.name
                );
                x
            })?;
        let buf = &memory.data(&self.store)[idx..(idx + len)];

        if buf.len() < len {
            return Err(LwskError::BufferTooSmall {
                expected: len,
                got: buf.len(),
            });
        }

        Ok(buf)
    }

    /// Get a mutable ref to the data backing a global in this functions Wasm module
    pub fn get_global_mut(&mut self, ident: &str, len: usize) -> Result<&mut [u8], LwskError> {
        let idx = self.get_global_idx(ident)? as usize;

        let mem_name = "memory";
        let memory = self
            .instance
            .get_memory(&self.store, mem_name)
            .ok_or(LwskError::NoSuchWasmMemory)
            .map_err(|x| {
                error!(
                    "no memory named {mem_name:?} was found in wasm module {:?}",
                    self.name
                );
                x
            })?;
        let buf = &mut memory.data_mut(&mut self.store)[idx..(idx + len)];

        if buf.len() < len {
            return Err(LwskError::BufferTooSmall {
                expected: len,
                got: buf.len(),
            });
        }

        Ok(buf)
    }
}
