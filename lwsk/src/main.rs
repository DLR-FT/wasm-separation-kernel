pub mod blueprint;

use log::{debug, info};

struct KernelConfig {
    ports: Vec<KPort>,
    functions: Vec<KFunction>,
}

struct KFunction {
    name: String,
    /// idx of a port
    consumes: Option<usize>,

    /// idx o a port it writes to
    produces: Option<usize>,

    /// parsed wasm of this function
    _wasm_mod: wasmi::Module,

    _engine: wasmi::Engine,

    store: wasmi::Store<()>,

    instance: wasmi::Instance,
}

struct KPort {
    /// name of this port
    name: String,

    /// buf backing up the data
    buf: Vec<u8>,
}

fn initialize_wasm() -> (wasmi::Engine, wasmi::Store<()>) {
    let mut config = wasmi::Config::default();
    config.consume_fuel(true);
    let engine = wasmi::Engine::new(&config);
    let store = wasmi::Store::new(&engine, ());
    (engine, store)
}

// TODO A function to commit the current state of a function for checkpointing

fn main() {
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    std::env::set_var("RUST_LOG", level.clone());

    pretty_env_logger::formatted_builder()
        .parse_filters(&level)
        .format_timestamp_secs()
        .init();

    info!("reading config");
    let bp = blueprint::Blueprint::new("example_blueprint.toml").unwrap();
    info!("configuring kernel");
    let mut kconfig = bp.to_kernel_config();

    // go through five major frames, call each function once in each maf
    for maf_n in 0..5 {
        info!("starting maf {maf_n}");
        for kf in kconfig.functions.iter_mut() {
            // set input if necessary
            if let Some(port_idx) = kf.consumes {
                debug!("{} -> {}.INPUT", kconfig.ports[port_idx].name, kf.name);

                let input_addr = kf
                    .instance
                    .get_global(&kf.store, "INPUT")
                    .unwrap()
                    .get(&kf.store)
                    .i32()
                    .unwrap();
                let wasm_memory = kf.instance.get_memory(&mut kf.store, "memory").unwrap();
                let host_input_buf = &kconfig.ports[port_idx].buf;
                let wasm_input_buf = &mut wasm_memory.data_mut(&mut kf.store)
                    [(input_addr as usize)..(input_addr + host_input_buf.len() as i32) as usize];
                debug!("host: {host_input_buf:?}\t wasm: {wasm_input_buf:?}");
                wasm_input_buf.copy_from_slice(&host_input_buf[..]);
                debug!("host: {host_input_buf:?}\t wasm: {wasm_input_buf:?}");
            }

            // call the function
            let ammount = 100_000;
            debug!("adding {ammount} fule to {}", kf.name);
            kf.store.add_fuel(ammount).unwrap();

            let process_data = kf
                .instance
                .get_typed_func::<(i32, i32), i32>(&kf.store, "process_data")
                .unwrap();
            let result = process_data.call(&mut kf.store, (0, 0)).unwrap();
            info!("calling {} yielded {result}", kf.name);

            debug!(
                "{} consumed {} fuel",
                kf.name,
                kf.store.fuel_consumed().unwrap()
            );

            // retrieve outputs if necessary
            if let Some(port_idx) = kf.produces {
                debug!("{}.OUTPUT -> {}", kf.name, kconfig.ports[port_idx].name);
                let output_addr = kf
                    .instance
                    .get_global(&kf.store, "OUTPUT")
                    .unwrap()
                    .get(&kf.store)
                    .i32()
                    .unwrap();
                let wasm_memory = kf.instance.get_memory(&mut kf.store, "memory").unwrap();
                let host_output_buf = &mut kconfig.ports[port_idx].buf;
                let wasm_output_buf = &wasm_memory.data(&kf.store)
                    [(output_addr as usize)..(output_addr + host_output_buf.len() as i32) as usize];

                debug!("host: {host_output_buf:?}\t wasm: {wasm_output_buf:?}");
                host_output_buf.copy_from_slice(wasm_output_buf);
                debug!("host: {host_output_buf:?}\t wasm: {wasm_output_buf:?}");
            }
        }
    }
}
