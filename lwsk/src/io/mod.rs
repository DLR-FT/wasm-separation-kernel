use crate::LwskError;

/// Driver for IO
///
pub trait IoDriver {
    /// Pull data from this IO source, if any
    ///
    /// If this driver has no new data present since the last call to [Self::pull], `buf` shall not be
    /// changed.
    fn pull(&mut self, buf: &mut [u8]) -> Result<(), LwskError>;

    /// Push data to this IO sink
    fn push(&mut self, buf: &[u8]) -> Result<(), LwskError>;
}

#[cfg(feature = "std")]
pub mod udp;
