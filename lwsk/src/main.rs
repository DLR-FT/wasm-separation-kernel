#[macro_use]
extern crate log;
use lwsk::blueprint;
use lwsk::schedule::ScheduleEntry;

// TODO A function to commit the current state of a function for checkpointing

#[cfg(feature = "std")]
mod cli;

#[cfg(feature = "std")]
fn main() {
    use lwsk::format_fuel_consumption;

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
    let mut kconfig = bp.to_kernel_config().unwrap();
    kconfig.validate().unwrap();

    if args.only_validate {
        return;
    }

    info!("entering main loop");
    loop {
        // get next schedule entry
        match kconfig.schedules[kconfig.current_schedule_idx].next_action() {
            ScheduleEntry::FunctionInvocation(function_idx) => {
                // get the corresponding kernel function
                let f = kconfig.functions.get_mut(function_idx).unwrap(); // TODO justify unwrap

                // set input if necessary
                if let Some(channel_idx) = f.consumes {
                    trace!(
                        "copying {:?}/channels[{channel_idx}] -> {:?}/functions[{function_idx}].INPUT",
                        kconfig.channels[channel_idx].name,
                        f.name
                    );

                    let host_input_buf = &kconfig.channels[channel_idx].buf;
                    if let Ok(wasm_input_buf) = f.get_global_mut("INPUT", host_input_buf.len()) {
                        wasm_input_buf.copy_from_slice(&host_input_buf[..]);
                    } else {
                        warn!("{:?}/functions[{function_idx}] has no INPUT", f.name);
                        continue;
                    }
                }

                // get the function
                let process_data = f.get_entry_function().unwrap();

                // refuel
                let amount = f.fuel_per_call; // TODO adjust fuel stuff
                trace!("refuel {:?}/functions[{function_idx}] to {amount}", f.name);
                f.store.set_fuel(amount).unwrap();
                let fuel_before = f.store.get_fuel().unwrap();

                // get current time
                let now = std::time::Instant::now();

                // call the function
                let result = process_data.call(&mut f.store, ()).unwrap();

                // time difference since before the call
                let duration = now.elapsed();

                // calculate fuel consumption
                let fuel_after = f.store.get_fuel().unwrap();
                let fuel_consumed = fuel_before - fuel_after;
                trace!(
                "{:?}/functions[{function_idx}] took {duration:?}, consumed {fuel_consumed} fuel", f.name);

                let (fuel_per_time, time_unit) = format_fuel_consumption(fuel_consumed, duration);

                debug!(
                    "burned {fuel_per_time} f/{time_unit}, taking {:?}/fuel",
                    duration.div_f32(fuel_consumed as f32)
                );

                // anounce the result
                debug!(
                    "calling {:?}/functions[{function_idx}] yielded {result}",
                    f.name
                );

                // retrieve outputs if necessary
                if let Some(channel_idx) = f.produces {
                    trace!(
                        "copying {:?}/functions[{function_idx}].INPUT -> {:?}/channels[{channel_idx}]",
                        f.name,
                        kconfig.channels[channel_idx].name,
                    );

                    let output_addr = f
                        .instance
                        .get_global(&f.store, "OUTPUT")
                        .unwrap()
                        .get(&f.store)
                        .i32()
                        .unwrap();
                    let wasm_memory = f.instance.get_memory(&mut f.store, "memory").unwrap();
                    let host_output_buf = &mut kconfig.channels[channel_idx].buf;
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
                let io_driver = &mut kconfig.io[from_io_idx];
                let memory = &mut kconfig.channels[to_channel_idx].buf;

                // ignore io erros apart from logging
                let _ = io_driver.pull(memory);
            }
            ScheduleEntry::IoOut {
                from_channel_idx,
                to_io_idx,
            } => {
                trace!("pushing data from channels[{from_channel_idx}] to io[{to_io_idx}]");
                let memory = &kconfig.channels[from_channel_idx].buf;
                debug!("{:#?}", kconfig.io.len());
                let io_driver = &mut kconfig.io[to_io_idx];
                // ignore io errors apart from logging
                let _ = io_driver.push(memory);
            }
            ScheduleEntry::Wait(duration) => std::thread::sleep(duration),
            ScheduleEntry::Schedule(new_schedule_idx) => {
                debug!(
                    "switch from schedule[{}] to schedule[{new_schedule_idx}]",
                    kconfig.current_schedule_idx
                );
                // set the next schedule id
                kconfig.current_schedule_idx = new_schedule_idx;
                // reset the schedule to its start
                kconfig.schedules[kconfig.current_schedule_idx].current_action = 0;
            }
        }
    }
}
