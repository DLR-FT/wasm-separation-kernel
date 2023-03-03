use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use wasmi::*;

fn main() -> Result<()> {
    // First step is to create the Wasm execution engine with some config.
    // In this example we are using the default configuration.
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let wat = r#"
        (module
            (import "host" "hello_native" (func $host_hello (param i32)))
            (func (export "hello") (param $arg i32)
                (call $host_hello (i32.add (local.get $arg) (i32.const 3 )))
            )
        )
    "#;
    // Wasmi does not yet support parsing `.wat` so we have to convert
    // out `.wat` into `.wasm` before we compile and validate it.
    let wasm = wat::parse_str(wat)?;
    let module = Module::new(&engine, &mut &wasm[..])?;
    // let module = Module::new(
    //     &engine,
    //     &include_bytes!("../../target/wasm32-unknown-unknown/release/partition_0.wasm")[..],
    // )?;

    let _sampling_ports: Arc<Mutex<HashMap<i32, Vec<u8>>>> = Arc::new(Mutex::new(HashMap::new()));

    // All Wasm objects operate within the context of a `Store`.
    // Each `Store` has a type parameter to store host-specific data,
    // which in this case we are using `42` for.
    type HostState = u32;
    let mut store = Store::new(&engine, 42);
    store.add_fuel(1_000_000).unwrap(); // add some fuel

    // let write_sampling_port = Func::wrap(
    //     &mut store,
    //     |mut caller: Caller<'_, HostState>, port_id: i32, value: i32| {
    //         caller.consume_fuel(1000).unwrap();
    //         let v = sampling_ports
    //             .lock()
    //             .expect("lock poised")
    //             .entry(port_id)
    //             .or_insert(Default::default());
    //     },
    // );

    let host_hello = Func::wrap(
        &mut store,
        |mut caller: Caller<'_, HostState>, param: i32| {
            caller.consume_fuel(50).unwrap();
            println!("Got {param} from WebAssembly");
            println!("My host state is: {}", caller.data());
        },
    );

    // In order to create Wasm module instances and link their imports
    // and exports we require a `Linker`.
    let mut linker = <Linker<HostState>>::new();
    // Instantiation of a Wasm module requires defining its imports and then
    // afterwards we can fetch exports by name, as well as asserting the
    // type signature of the function with `get_typed_func`.
    //
    // Also before using an instance created this way we need to start it.
    linker.define("host", "hello_native", host_hello)?;
    let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;
    // let hello = instance.get_typed_func::<u32, ()>(&store, "hello")?;
    // let adder = instance.get_typed_func::<(i32, i32), i32>(&store, "adder")?;
    let process_data = instance.get_typed_func::<(i32, i32), i32>(&store, "process_data")?;
    // let wasm_alloc = instance.get_typed_func::<i32, i32>(&store, "alloc")?;
    let my_memory = instance
        .get_global(&store, "MY_MEMORY")
        .unwrap()
        .get(&store)
        .i32()
        .unwrap() as usize;
    let memory = instance.get_memory(&mut store, "memory").unwrap();
    let my_memory_slice = &mut memory.data_mut(&mut store)[my_memory..my_memory + 32];
    my_memory_slice[0] = 10;
    my_memory_slice[1] = 11;
    my_memory_slice[2] = 12;
    my_memory_slice[3] = 13;

    // let address = wasm_alloc.call(&mut store, 5).unwrap();
    // panic!("{address}");

    // let result = adder.call(&mut store, (5, 6));
    // println!("adder result: {result:?}");

    // my_memory.data_mut(&mut store)[0] = 13;
    let result = process_data.call(&mut store, (my_memory as i32, 4));
    println!("process_data result: {result:?}");

    println!("consumed fuel: {:?}", store.fuel_consumed());

    Ok(())
}
