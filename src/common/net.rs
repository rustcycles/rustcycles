use std::{
    collections::VecDeque,
    io::{self, ErrorKind, Read, Write},
    iter, mem,
    net::TcpStream,
};

use serde::{de::DeserializeOwned, Serialize};

type MsgLen = u32;
const HEADER_LEN: usize = mem::size_of::<MsgLen>();

#[derive(Debug)]
pub(crate) struct NetworkMessage {
    content_len: [u8; HEADER_LEN],
    buf: Vec<u8>,
}

pub(crate) fn serialize<M>(message: M) -> NetworkMessage
where
    M: Serialize,
{
    let buf = bincode::serialize(&message).expect("bincode failed to serialize message");
    let content_len = MsgLen::try_from(buf.len())
        .unwrap_or_else(|err| {
            panic!("bincode message length ({} bytes) overflowed its type: {:?}", buf.len(), err)
        })
        .to_le_bytes();
    NetworkMessage { content_len, buf }
}

pub(crate) fn send(
    network_message: &NetworkMessage,
    stream: &mut TcpStream,
) -> Result<(), io::Error> {
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

/// Read all available bytes from `stream` into `buffer`,
/// parse messages that are complete and return them in a vector.
///
/// Also return whether the connection has been closed (doesn't matter if cleanly or reading failed).
#[must_use]
pub(crate) fn receive<M>(stream: &mut TcpStream, buffer: &mut VecDeque<u8>) -> (Vec<M>, bool)
where
    M: DeserializeOwned,
{
    let closed = read(stream, buffer);
    let messages = iter::from_fn(|| parse_one(buffer)).collect();
    (messages, closed)
}

/// Read all available bytes from `stream` into `buffer`,
/// parse a single message if there is enough data and return the message or None.
///
/// Also return whether the connection has been closed (doesn't matter if cleanly or reading failed).
#[must_use]
pub(crate) fn receive_one<M>(stream: &mut TcpStream, buffer: &mut VecDeque<u8>) -> (Option<M>, bool)
where
    M: DeserializeOwned,
{
    let closed = read(stream, buffer);
    let msg = parse_one(buffer);
    (msg, closed)
}

/// Read all available bytes until the stream would block.
fn read(stream: &mut TcpStream, buffer: &mut VecDeque<u8>) -> bool {
    // LATER Test networking thoroughly
    //      - lossy and slow connections
    //      - fragmented and merged packets
    // LATER(security) Test large amounts of data
    let mut closed = false;
    loop {
        // No particular reason for the buffer size, except BufReader uses the same.
        let mut buf = [0; 8192];
        let res = stream.read(&mut buf);
        match res {
            Ok(0) => {
                // The connection has been closed, don't get stuck in this loop.
                // This can happen for example when the server crashes.
                dbg_logf!("Connection closed when reading");
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
                dbg_logf!("Connection closed when reading - error: {}", e);
                closed = true;
                break;
            }
        }
    }
    closed
}

/// Parse a message from `buffer` or return None if there's not enough data.
fn parse_one<M>(buffer: &mut VecDeque<u8>) -> Option<M>
where
    M: DeserializeOwned,
{
    if buffer.len() < HEADER_LEN {
        return None;
    }

    // There's no convenient way to make this generic over msg len 2 and 4,
    // just keep one version commented out.
    //let len_bytes = [buffer[0], buffer[1]];
    //let content_len = usize::from(MsgLen::from_le_bytes(len_bytes));
    let len_bytes = [buffer[0], buffer[1], buffer[2], buffer[3]];
    let content_len = usize::try_from(MsgLen::from_le_bytes(len_bytes)).unwrap();

    if buffer.len() < HEADER_LEN + content_len {
        // Not enough bytes in buffer for a full message.
        return None;
    }

    buffer.drain(0..HEADER_LEN);
    let bytes: Vec<_> = buffer.drain(0..content_len).collect();
    let message = bincode::deserialize(&bytes).unwrap();

    Some(message)
}
