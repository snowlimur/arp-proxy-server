use socket2::{Domain, Socket, Type};
use std::net::{AddrParseError, SocketAddr};

pub fn listen_reuse_socket(addr: &SocketAddr) -> Result<Socket, std::io::Error> {
    let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_port(true)?;
    socket.set_reuse_address(true)?;
    socket.bind(&(*addr).into())?;
    socket.listen(128)?;
    Ok(socket)
}

pub fn parse_address(mut addr: String) -> Result<SocketAddr, AddrParseError> {
    if addr.starts_with(':') {
        addr.insert_str(0, "0.0.0.0");
    }

    addr.parse()
}
