#![cfg_attr(not(any(test, feature = "std")), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod blueprint;
pub mod io;
pub mod kernel;
pub mod schedule;

pub use kernel::*;

#[macro_use]
extern crate log;

#[derive(Debug)]
pub enum LwskError {
    IoChannelCreationError,

    WasmLoadError,

    UnexpectedWasmType,

    GlobalDoesNotExist,

    // The specified memory was not found
    NoSuchWasmMemory,

    BufferTooSmall { expected: usize, got: usize },

    DriverError(i64),

    InvalidFunctionIdx(usize),
    InvalidChannelIdx(usize),
    InvalidIoIdx(usize),
}

#[cfg(feature = "std")]
impl From<std::io::Error> for LwskError {
    fn from(_value: std::io::Error) -> Self {
        Self::IoChannelCreationError
    }
}

pub fn format_fuel_consumption(
    fuel_consumed: u64,
    duration: core::time::Duration,
) -> (f32, &'static str) {
    let duration_nanos = duration.as_nanos();

    match (fuel_consumed, duration_nanos) {
        _ => (
            fuel_consumed as f32 / (duration.as_nanos() as f32 / 1e3),
            "μs",
        ),
    }
}
