//! UDP driver based on Rust's std library

use crate::LwskError;

pub struct Udp {
    socket: std::net::UdpSocket,
}

impl Udp {
    pub fn new<A: std::net::ToSocketAddrs, B: std::net::ToSocketAddrs>(
        addr: A,
        connect: B,
    ) -> Result<Self, LwskError> {
        let socket = std::net::UdpSocket::bind(addr)?;
        socket.connect(connect)?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket })
    }
}

impl super::IoDriver for Udp {
    fn pull(&mut self, buf: &mut [u8]) -> Result<(), LwskError> {
        match self.socket.recv(buf) {
            Ok(n) => log::debug!("received {n} bytes from UDP"),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                log::debug!("no new message in UDP port")
            }
            Err(e) => {
                log::error!("could not receive from UDP socket: {e}");
                return Err(LwskError::DriverError(e.raw_os_error().unwrap().into()));
            }
        }
        Ok(())
    }

    fn push(&mut self, buf: &[u8]) -> Result<(), LwskError> {
        match self.socket.send(buf) {
            Ok(n) => log::debug!("wrote {n} byte to UDP"),
            Err(e) => {
                log::error!("could not send to UDP socket: {e}");
                return Err(LwskError::DriverError(e.raw_os_error().unwrap().into()));
            }
        }
        Ok(())
    }
}
