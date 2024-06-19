#[macro_use]
extern crate log;
use lwsk::blueprint;
use lwsk::schedule::ScheduleEntry;

// TODO A function to commit the current state of a function for checkpointing

#[cfg(feature = "std")]
mod cli;

#[cfg(feature = "std")]
fn main() {
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    std::env::set_var("RUST_LOG", level.clone());

    pretty_env_logger::formatted_builder()
        .parse_filters(&level)
        .format_timestamp_secs()
        .init();

    let args: cli::Args = clap::Parser::parse();

    info!("reading config");
    let maybe_bp = blueprint::Blueprint::new(args.blueprint);
    let bp = match maybe_bp {
        Ok(bp) => bp,
        Err(e) => {
            error!("{e}");
            panic!("");
        }
    };

    info!("configuring kernel");
    let mut kconfig = bp.to_kernel_config();
    kconfig.validate().unwrap();

    if args.only_validate {
        return;
    }

    let mut schedule_idx = 0;

    let current_schedule = kconfig.schedules.get_mut(0).unwrap();

    info!("entering main loop");
    loop {
        // get next schedule entry
        match current_schedule.get_mut(schedule_idx).unwrap() {
            ScheduleEntry::FunctionInvocation(function_idx) => {
                // get the corresponding kernel function
                let f = kconfig.functions.get_mut(*function_idx).unwrap(); // TODO justify unwrap

                // set input if necessary
                if let Some(port_idx) = f.consumes {
                    debug!("{} -> {}.INPUT", kconfig.channels[port_idx].name, f.name);

                    let host_input_buf = &kconfig.channels[port_idx].buf;
                    if let Ok(wasm_input_buf) = f.get_global_mut("INPUT", host_input_buf.len()) {
                        wasm_input_buf.copy_from_slice(&host_input_buf[..]);
                    } else {
                        warn!("partition {:?} has no INPUT", f.name);
                        continue;
                    }
                }

                // call the function
                let amount = 1_500_000; // TODO adjust fuel stuff
                trace!("set fuel to {amount}");
                f.store.set_fuel(amount).unwrap();

                let process_data = f
                    .instance
                    .get_typed_func::<(i32, i32), i32>(&f.store, "process_data")
                    .unwrap();

                let fuel_before = f.store.get_fuel().unwrap();
                let now = std::time::Instant::now();
                let result = process_data.call(&mut f.store, (0, 0)).unwrap();
                let duration = now.elapsed();
                let fuel_after = f.store.get_fuel().unwrap();
                let fuel_consumed = fuel_after as i64 - fuel_before as i64;
                trace!(
                "Time elapsed {duration:?}, fuel consumed {fuel_consumed}, time per 1k fuel {:?}",
                duration.div_f32(fuel_consumed as f32 / 1e3)
            );

                debug!("calling {} yielded {result}", f.name);

                trace!(
                    "functions[{function_idx}] {} consumed {} fuel",
                    f.name,
                    f.store.get_fuel().unwrap()
                );

                // retrieve outputs if necessary
                if let Some(port_idx) = f.produces {
                    debug!("{}.OUTPUT -> {}", f.name, kconfig.channels[port_idx].name);
                    let output_addr = f
                        .instance
                        .get_global(&f.store, "OUTPUT")
                        .unwrap()
                        .get(&f.store)
                        .i32()
                        .unwrap();
                    let wasm_memory = f.instance.get_memory(&mut f.store, "memory").unwrap();
                    let host_output_buf = &mut kconfig.channels[port_idx].buf;
                    let wasm_output_buf = &wasm_memory.data(&f.store)[(output_addr as usize)
                        ..(output_addr + host_output_buf.len() as i32) as usize];

                    host_output_buf.copy_from_slice(wasm_output_buf);
                }
            }
            ScheduleEntry::IoIn {
                from_io_idx,
                to_channel_idx,
            } => {
                trace!("pulling data from io[{from_io_idx}] to channels[{to_channel_idx}]");
                let io_driver = &mut kconfig.io[*from_io_idx];
                let memory = &mut kconfig.channels[*to_channel_idx].buf;

                // ignore io erros apart from logging
                let _ = io_driver.pull(memory);
            }
            ScheduleEntry::IoOut {
                from_channel_idx,
                to_io_idx,
            } => {
                trace!("pushing data from channels[{from_channel_idx}] to io[{to_io_idx}]");
                let memory = &kconfig.channels[*from_channel_idx].buf;
                debug!("{:#?}", kconfig.io.len());
                let io_driver = &mut kconfig.io[*to_io_idx];
                // ignore io errors apart from logging
                let _ = io_driver.push(memory);
            }
            ScheduleEntry::Wait(duration) => std::thread::sleep(*duration),
        }
        schedule_idx = (schedule_idx + 1) % current_schedule.len();
    }
}
