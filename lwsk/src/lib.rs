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

pub type LwskResult<T> = Result<T, LwskError>;

// TODO impl Error texts
#[derive(Debug, thiserror::Error)]
pub enum LwskError {
    #[error("TODO")]
    IoChannelCreationError,

    #[error("TODO")]
    WasmLoadError,

    #[error("TODO")]
    UnexpectedWasmType,

    #[error("TODO")]
    GlobalDoesNotExist,

    #[error("TODO")]
    EmptySchedule,

    #[error("The specified memory was not found")]
    NoSuchWasmMemory,

    #[error("The buffer is to small. Got {got}, expected at least {expected}")]
    BufferTooSmall { expected: usize, got: usize },

    #[error("TODO")]
    DriverError(i64),

    #[error("TODO")]
    InvalidFunctionIdx(usize),
    #[error("TODO")]
    InvalidChannelIdx(usize),
    #[error("TODO")]
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

    (
        fuel_consumed as f32 / (duration.as_nanos() as f32 / 1e3),
        "μs",
    )
}
