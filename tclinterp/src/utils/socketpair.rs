use std::{
    net::{TcpListener, TcpStream},
    thread,
};

use rand::Rng;

use crate::{channel::Channel, error::TclError, interp::TclInterp};

/// Create a two-way communication channel between Rust and Tcl.
pub fn create_socketpair(interp: TclInterp) -> Result<(TcpStream, Channel), TclError> {
    let host = "127.0.0.1";
    let port = rand::thread_rng().gen_range(1024, 8096);

    let server = TcpListener::bind((host, port)).map_err(|_| unimplemented!())?;
    let accept_thread = thread::spawn(move || server.accept().map(|(sock, _)| sock));

    let tcl_sock = Channel::open_tcp_client(interp, host, port)?;

    let rust_sock = accept_thread
        .join()
        .map_err(|_| TclError::new("Error joining thread"))?
        .map_err(|_| TclError::new("Error creating channel"))?;

    Ok((rust_sock, tcl_sock))
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};

    use super::*;

    #[test]
    fn test_creation() {
        assert!(create_socketpair(TclInterp::new().unwrap()).is_ok());
    }

    #[test]
    fn test_tclsock_recv_data() {
        let (mut rust, mut tcl) = create_socketpair(TclInterp::new().unwrap()).unwrap();
        write!(rust, "\0hello, \0world\0").unwrap();

        let mut data: Vec<u8> = Default::default();
        tcl.read_to_end(&mut data).unwrap();

        assert_eq!(String::from_utf8(data).unwrap(), "\0hello, \0world\0");
    }

    #[test]
    fn test_tclsock_recv_empty() {
        let (_, mut tcl) = create_socketpair(TclInterp::new().unwrap()).unwrap();

        let mut data: String = Default::default();
        tcl.read_to_string(&mut data).unwrap();

        assert_eq!(data, "");
    }

    #[test]
    fn test_tclsock_send() {
        let (rust, mut tcl) = create_socketpair(TclInterp::new().unwrap()).unwrap();
        write!(tcl, "\0hello, \0world\0\n").unwrap();
        tcl.flush().unwrap();

        let mut data: String = Default::default();
        BufReader::new(rust).read_line(&mut data).unwrap();

        assert_eq!(data, "\0hello, \0world\0\n");
    }
}
