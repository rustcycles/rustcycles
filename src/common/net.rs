use std::{
    collections::VecDeque,
    error::Error,
    io::{ErrorKind, Read, Write},
    net::TcpStream,
};

use serde::{de::DeserializeOwned, Serialize};

const HEADER_LEN: usize = 2;

#[derive(Debug)]
pub(crate) struct NetworkMessage {
    content_len: [u8; HEADER_LEN],
    buf: Vec<u8>,
}

pub(crate) fn serialize<M>(message: M) -> NetworkMessage
where
    M: Serialize,
{
    let buf = bincode::serialize(&message).unwrap();
    let content_len = u16::try_from(buf.len()).unwrap().to_le_bytes();
    NetworkMessage { content_len, buf }
}

pub(crate) fn send(
    network_message: &NetworkMessage,
    stream: &mut TcpStream,
) -> Result<(), Box<dyn Error>> {
    // LATER Measure network usage.
    // LATER Try to minimize network usage.
    //       General purpose compression could help a bit,
    //       but using what we know about the data should give much better results.

    // Prefix data by length so it's easy to parse on the other side.
    stream.write_all(&network_message.content_len)?;
    stream.write_all(&network_message.buf)?;
    stream.flush()?; // LATER No idea if necessary or how it interacts with set_nodelay

    Ok(())
}

/// Read bytes from `stream` into `buffer`,
/// parse packets that are complete and add them to `packets`.
///
/// Returns whether the connection has been closed (doesn't matter if cleanly or reading failed).
#[must_use]
pub(crate) fn receive<P>(
    stream: &mut TcpStream,
    buffer: &mut VecDeque<u8>,
    packets: &mut Vec<P>,
) -> bool
where
    P: DeserializeOwned,
{
    // Read all available bytes until the stream would block.
    // LATER Test networking thoroughly
    //      - large amounts of data
    //      - lossy and slow connections
    //      - fragmented and merged packets
    let mut closed = false;
    loop {
        // No particular reason for the buffer size, except BufReader uses the same.
        let mut buf = [0; 8192];
        let res = stream.read(&mut buf);
        match res {
            Ok(0) => {
                // The connection has been closed, don't get stuck in this loop.
                // This can happen for example when the server crashes.
                closed = true;
                break;
            }
            Ok(n) => {
                buffer.extend(&buf[0..n]);
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                break;
            }
            Err(e) => {
                println!("network error (read): {}", e);
                closed = true;
                break;
            }
        }
    }

    // Parse the received bytes
    loop {
        if buffer.len() < HEADER_LEN {
            break;
        }
        let len_bytes = [buffer[0], buffer[1]];
        let content_len = usize::from(u16::from_le_bytes(len_bytes));
        if buffer.len() < HEADER_LEN + content_len {
            // Not enough bytes in buffer for a full frame.
            break;
        }
        buffer.pop_front();
        buffer.pop_front();
        let bytes: Vec<_> = buffer.drain(0..content_len).collect();
        let message = bincode::deserialize(&bytes).unwrap();
        packets.push(message);
    }

    closed
}
