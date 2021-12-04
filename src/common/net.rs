use std::{
    collections::VecDeque,
    io::{ErrorKind, Read},
    net::TcpStream,
};

pub(crate) fn receive<P>(stream: &mut TcpStream, buffer: &mut VecDeque<u8>, packets: &mut Vec<P>)
where
    for<'de> P: serde::Deserialize<'de>,
{
    // Read all available bytes until the stream would block.
    // LATER Test networking thoroughly
    //      - large amounts of data
    //      - lossy and slow connections
    //      - fragmented and merged packets
    // TODO Err(ref e) if e.kind() == ErrorKind::Interrupted => {} ???
    loop {
        // No particular reason for the buffer size, except BufReader uses the same.
        let mut buf = [0; 8192];
        let res = stream.read(&mut buf);
        match res {
            Ok(0) => {
                // The connection has been closed, don't get stuck in this loop.
                // This can happen for example when the server crashes.
                // LATER Some kind of clean client shutdown.
                //  Currently the client crashes later when attempting to send.
                break;
            }
            Ok(n) => {
                buffer.extend(&buf[0..n]);
            }
            Err(err) => match err.kind() {
                ErrorKind::WouldBlock => {
                    break;
                }
                _ => panic!("network error (read): {}", err),
            },
        }
    }

    // Parse the received bytes
    loop {
        if buffer.len() < 2 {
            break;
        }
        let len_bytes = [buffer[0], buffer[1]];
        let len = usize::from(u16::from_le_bytes(len_bytes));
        if buffer.len() < len + 2 {
            // Not enough bytes in buffer for a full frame.
            break;
        }
        buffer.pop_front();
        buffer.pop_front();
        let bytes: Vec<_> = buffer.drain(0..len).collect();
        let message = bincode::deserialize(&bytes).unwrap();
        packets.push(message);
    }
}
