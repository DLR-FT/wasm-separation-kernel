use crate::LwskError;

pub trait IoDriver {
    // pull data from this port, if any
    fn pull(&mut self, buf: &mut [u8]) -> Result<(), LwskError>;

    // push data to this port
    fn push(&mut self, buf: &[u8]) -> Result<(), LwskError>;
}

#[cfg(feature = "std")]
pub mod udp;
