#[macro_use]
extern crate log;
use lwsk::blueprint;

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
                // for x in kf.instance.exports(&kf.store) {
                //     println!("{x:#?}");
                // }

                let host_input_buf = &kconfig.ports[port_idx].buf;
                if let Ok(wasm_input_buf) = kf.get_global_mut("INPUT", host_input_buf.len()) {
                    wasm_input_buf.copy_from_slice(&host_input_buf[..]);
                } else {
                    warn!("partition {:?} has no INPUT", kf.name);
                    continue;
                }
            }

            // call the function
            let ammount = 15_000;
            debug!("adding {ammount} fuel to {}", kf.name);
            kf.store.add_fuel(ammount).unwrap();

            let process_data = kf
                .instance
                .get_typed_func::<(i32, i32), i32>(&kf.store, "process_data")
                .unwrap();

            let fuel_before = kf.store.fuel_consumed().unwrap();
            let now = std::time::Instant::now();
            let result = process_data.call(&mut kf.store, (0, 0)).unwrap();
            let duration = now.elapsed();
            let fuel_after = kf.store.fuel_consumed().unwrap();
            let fuel_consumed = fuel_after - fuel_before;
            debug!(
                "Time elapsed {duration:?}, fuel consumed {fuel_consumed}, time per 1k fuel {:?}",
                duration.div_f32(fuel_consumed as f32 / 1e3)
            );

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

                host_output_buf.copy_from_slice(wasm_output_buf);
            }
        }
    }
}
