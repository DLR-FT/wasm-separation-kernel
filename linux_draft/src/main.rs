use anyhow::{anyhow, Result};
use wasmi::*;
use wat;

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
    let wasm = wat::parse_str(&wat)?;
    let module = Module::new(&engine, &mut &wasm[..])?;

    // All Wasm objects operate within the context of a `Store`.
    // Each `Store` has a type parameter to store host-specific data,
    // which in this case we are using `42` for.
    type HostState = u32;
    let mut store = Store::new(&engine, 42);
    store.add_fuel(5_500).unwrap(); // add some fuel

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
    let hello = instance.get_typed_func::<u32, ()>(&store, "hello")?;

    let linear_memory = Memory::new(&mut store, MemoryType::new(16, Some(128)).unwrap()); // b"Hello Punkt";

    // And finally we can call the wasm!
    let result = hello.call(&mut store, u32::MAX - 100);
    println!("{result:#?}");

    let result = hello.call(&mut store, 13);
    println!("{result:#?}");

    println!("consumed fuel: {:?}", store.fuel_consumed());

    Ok(())
}
