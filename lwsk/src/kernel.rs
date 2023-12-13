use crate::LwskError;

pub struct KernelConfig {
    pub ports: Vec<KPort>,
    pub functions: Vec<EphemeralPartition>,
    // io: Vec<Udp>,
}

pub struct KernelState {
    // last_io_idx:
}

pub struct EphemeralPartition {
    pub name: String,
    /// idx of a port
    pub consumes: Option<usize>,

    /// idx o a port it writes to
    pub produces: Option<usize>,

    /// parsed wasm of this function
    pub _wasm_mod: wasmi::Module,

    pub _engine: wasmi::Engine,

    pub store: wasmi::Store<()>,

    pub instance: wasmi::Instance,
}

pub struct KPort {
    /// name of this port
    pub name: String,

    /// buf backing up the data
    pub buf: Vec<u8>,
}

impl KernelConfig {
    // fn pull_all_io(&mut self) {
    //     for channel in self.io {
    //         // channel.sample(self.ports[channel.target_port_idx]);
    //     }
    // }

    // fn event_loop() {
    //     loop {
    //         // self.pull_all_io();
    //     }
    // }
}

pub fn initialize_wasm() -> (wasmi::Engine, wasmi::Store<()>) {
    let mut config = wasmi::Config::default();
    config.consume_fuel(true);
    let engine = wasmi::Engine::new(&config);
    let store = wasmi::Store::new(&engine, ());
    (engine, store)
}

impl EphemeralPartition {
    pub fn load(name: &str, wasm_module_path: &str) -> Result<Self, LwskError> {
        trace!("loading partition {name:?} from {wasm_module_path:?}");

        #[cfg(feature = "std")]
        let wasm_file = match std::fs::File::open(&wasm_module_path) {
            Ok(file) => file,
            Err(e) => {
                error!("could not open file {wasm_module_path:?}: {e}");
                return Err(LwskError::WasmLoadError);
            }
        };

        #[cfg(not(feature = "std"))]
        let wasm_file = {
            todo!("interprete the path as resource descriptor for a fit image or something, get a byte slice, be done with it")
        };

        let (engine, mut store) = super::initialize_wasm();

        trace!("parsing wasm file");
        let module = match wasmi::Module::new(&engine, wasm_file) {
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
        })
    }

    /// Get the index of a global inside this partitions wasm module
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

    /// Get a shared ref to the memory backing a global in this partitions wasm module
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

    /// Get a mutable ref to the data backing a global in this partitions wasm module
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
