use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};

use rand::Rng;

use crate::tclsocket::TclSocket;

use super::*;

fn create_channel(interp: TclInterp) -> Result<(TcpStream, TclSocket), TclError> {
    let host = "127.0.0.1";
    let port = rand::thread_rng().gen_range(1024, 8096);

    let server = TcpListener::bind((host, port)).map_err(|_| unimplemented!())?;
    let accept_thread = thread::spawn(move || server.accept().map(|(sock, _)| sock));

    let tcl_sock = TclSocket::connect(interp, host, &port.to_string())?;

    let rust_sock = accept_thread
        .join()
        .map_err(|_| TclError::new("Error joining thread"))?
        .map_err(|_| TclError::new("Error creating channel"))?;

    Ok((rust_sock, tcl_sock))
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};

    use super::*;

    #[test]
    fn test_creation() {
        assert!(create_channel(TclInterp::new().unwrap()).is_ok());
    }

    #[test]
    fn test_tclsock_recv_data() {
        let (mut rust, mut tcl) = create_channel(TclInterp::new().unwrap()).unwrap();
        write!(rust, "hello, world").unwrap();

        let mut data: String = Default::default();
        tcl.read_to_string(&mut data).unwrap();

        assert_eq!(data, "hello, world");
    }

    #[test]
    fn test_tclsock_recv_empty() {
        let (_, mut tcl) = create_channel(TclInterp::new().unwrap()).unwrap();

        let mut data: String = Default::default();
        tcl.read_to_string(&mut data).unwrap();

        assert_eq!(data, "");
    }

    #[test]
    fn test_tclsock_send() {
        let (rust, mut tcl) = create_channel(TclInterp::new().unwrap()).unwrap();
        write!(tcl, "hello, world\n").unwrap();
        tcl.flush().unwrap();

        let mut data: String = Default::default();
        BufReader::new(rust).read_line(&mut data).unwrap();

        assert_eq!(data, "hello, world\n");
    }
}
