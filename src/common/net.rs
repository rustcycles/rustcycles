//! Networking listeners and connections. TCP (remote) and mpsc (local).
//!
//! We could use TCP locally too but WASM doesn't support it so we use mpsc.
//!
//! LATER Long term the plan is to switch away from TCP but honestly as long as it works, it's not a priority.
//! The common wisdom to never use TCP for games doesn't seem to apply on modern networks.
//! Veloren has been using TCP for years and nobody complains because nobody even notices.

// This file is shared between RecWars and RustCycles
// to keep their debug APIs the same
// and as an experiment to see how much code is shareable
// between significantly different multiplayer games.

use std::{
    io::{self, ErrorKind, Read, Write},
    iter, mem,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::mpsc::{Receiver, Sender, TryRecvError},
    thread,
    time::Duration,
};

use serde::de::DeserializeOwned;

use crate::prelude::*;

/// A trait to abstract over local and remote listeners.
///
/// Note: ideally only the function would be generic over the message type
/// but then the trait wouldn't be object safe so we have to make the whole trait genetic.
/// The same reasning applies to `Connection<M>`.
pub trait Listener<M>
where
    M: DeserializeOwned,
{
    fn accept_conn(&mut self) -> io::Result<Box<dyn Connection<M>>>;
}

pub struct LocalListener {
    conn: Option<LocalConnection>,
}

impl LocalListener {
    pub fn new(conn: LocalConnection) -> Self {
        Self { conn: Some(conn) }
    }
}

impl<M> Listener<M> for LocalListener
where
    M: DeserializeOwned,
{
    fn accept_conn(&mut self) -> io::Result<Box<dyn Connection<M>>> {
        let conn = self.conn.take();
        match conn {
            Some(conn) => Ok(Box::new(conn)),
            None => Err(io::Error::new(ErrorKind::WouldBlock, "dummy")),
        }
    }
}

// Note we use the TcpListener from std here, not a custom type,
// no point adding an extra type.
impl<M> Listener<M> for TcpListener
where
    M: DeserializeOwned,
{
    fn accept_conn(&mut self) -> io::Result<Box<dyn Connection<M>>> {
        let (stream, addr) = self.accept()?;

        // LATER Measure if nodelay actually makes a difference,
        // or better yet, replace TCP with something better.
        // Same on the client.
        // Also how does it interact with flushing the stram after each write?
        stream.set_nodelay(true).unwrap();
        stream.set_nonblocking(true).unwrap();

        let conn = TcpConnection::new(stream, addr);
        Ok(Box::new(conn))
    }
}

type MsgLen = u32;
const HEADER_LEN: usize = mem::size_of::<MsgLen>();

#[derive(Debug, Clone)]
pub struct NetworkMessage {
    content_len: [u8; HEADER_LEN],
    buf: Vec<u8>,
}

/// A trait to abstract over local and remove connections.
pub trait Connection<M>
where
    M: DeserializeOwned,
{
    fn send(&mut self, net_msg: &NetworkMessage) -> Result<(), io::Error>;

    // `#[must_use]` only does something in the trait definition,
    // no need to repeat it in the impls:
    // https://github.com/rust-lang/rust/issues/48486

    /// Read all available messages and return them.
    ///
    /// Also return whether the connection has been closed (doesn't matter if cleanly or reading failed).
    #[must_use]
    fn receive(&mut self) -> (Vec<M>, bool);

    /// Read one message if available or return None.
    ///
    /// Also return whether the connection has been closed (doesn't matter if cleanly or reading failed).
    #[must_use]
    fn receive_one(&mut self) -> (Option<M>, bool);

    #[must_use]
    fn addr(&self) -> String;
}

/// Send and receive serialized messages locally using mpsc.
///
/// It would be more efficient to avoid serialization entirely but:
/// - It would require a bigger redesign.
/// - We need to serialize them for demos/replays anyway.
pub struct LocalConnection {
    pub sender: Sender<NetworkMessage>,
    pub receiver: Receiver<NetworkMessage>,
}

impl LocalConnection {
    pub fn new(sender: Sender<NetworkMessage>, receiver: Receiver<NetworkMessage>) -> Self {
        Self { sender, receiver }
    }
}

impl<M> Connection<M> for LocalConnection
where
    M: DeserializeOwned,
{
    fn send(&mut self, net_msg: &NetworkMessage) -> Result<(), io::Error> {
        self.sender.send(net_msg.clone()).unwrap();
        Ok(())
    }

    fn receive(&mut self) -> (Vec<M>, bool) {
        let mut msgs = Vec::new();
        loop {
            let (msg, closed) = self.receive_one();
            if let Some(msg) = msg {
                msgs.push(msg);
            } else {
                // If closed is ever gonna be true,
                // it's gonna be on the last iteraton
                // so it doesn't matter we throw away the earlier values.
                return (msgs, closed);
            }
        }
    }

    fn receive_one(&mut self) -> (Option<M>, bool) {
        let res = self.receiver.try_recv();
        match res {
            Ok(msg) => {
                let msg = bincode::deserialize(&msg.buf).unwrap();
                (Some(msg), false)
            }
            Err(TryRecvError::Empty) => (None, false),
            Err(TryRecvError::Disconnected) => (None, true),
        }
    }

    fn addr(&self) -> String {
        "local".to_owned()
    }
}

pub struct TcpConnection {
    stream: TcpStream,
    buffer: VecDeque<u8>,
    pub addr: SocketAddr,
}

/// Send and receive serialized messages over the network using TCP.
impl TcpConnection {
    pub fn new(stream: TcpStream, addr: SocketAddr) -> Self {
        Self {
            stream,
            buffer: VecDeque::new(),
            addr,
        }
    }
}

impl<M> Connection<M> for TcpConnection
where
    M: DeserializeOwned,
{
    fn send(&mut self, net_msg: &NetworkMessage) -> Result<(), io::Error> {
        // LATER Measure network usage.
        // LATER Try to minimize network usage.
        //       General purpose compression could help a bit,
        //       but using what we know about the data should give much better results.

        // Prefix data by length so it's easy to parse on the other side.
        self.stream.write_all(&net_msg.content_len)?;
        self.stream.write_all(&net_msg.buf)?;
        self.stream.flush()?; // LATER No idea if necessary or how it interacts with set_nodelay

        Ok(())
    }

    /// Read all available bytes from `stream` into `buffer`,
    /// parse messages that are complete and return them in a vector.
    ///
    /// Also return whether the connection has been closed (doesn't matter if cleanly or reading failed).
    fn receive(&mut self) -> (Vec<M>, bool) {
        let closed = read(&mut self.stream, &mut self.buffer);
        let msgs = iter::from_fn(|| parse_one(&mut self.buffer)).collect();
        (msgs, closed)
    }

    /// Read all available bytes from `stream` into `buffer`,
    /// parse a single message if there is enough data and return the message or None.
    ///
    /// Also return whether the connection has been closed (doesn't matter if cleanly or reading failed).
    fn receive_one(&mut self) -> (Option<M>, bool) {
        let closed = read(&mut self.stream, &mut self.buffer);
        let msg = parse_one(&mut self.buffer);
        (msg, closed)
    }

    fn addr(&self) -> String {
        self.addr.to_string()
    }
}

pub fn tcp_connect(cvars: &Cvars, addr: &str) -> TcpConnection {
    let addr = SocketAddr::from_str(addr).unwrap();

    let mut connect_attempts = 0;
    let stream = loop {
        connect_attempts += 1;
        // LATER Don't block the main thread - async? just try again next iteration of the main/game loop?
        // LATER Limit the number of attempts.
        if let Ok(stream) = TcpStream::connect(addr) {
            dbg_logf!("connect attempts: {}", connect_attempts);
            break stream;
        }
        if connect_attempts % cvars.cl_net_connect_retry_print_every_n == 0 {
            dbg_logf!("connect attempts: {}", connect_attempts);
        }
        thread::sleep(Duration::from_millis(cvars.cl_net_connect_retry_delay_ms));
    };
    stream.set_nodelay(true).unwrap();
    stream.set_nonblocking(true).unwrap();

    TcpConnection::new(stream, addr)
}

pub fn serialize<M>(msg: M) -> NetworkMessage
where
    M: Serialize,
{
    let buf = bincode::serialize(&msg).expect("bincode failed to serialize message");
    let content_len = MsgLen::try_from(buf.len())
        .unwrap_or_else(|err| {
            panic!("bincode message length ({} bytes) overflowed its type: {:?}", buf.len(), err)
        })
        .to_le_bytes();
    NetworkMessage { content_len, buf }
}

/// Read all available bytes until the stream would block.
fn read(stream: &mut TcpStream, buffer: &mut VecDeque<u8>) -> bool {
    // LATER Test networking thoroughly
    //      - lossy and slow connections
    //      - fragmented and merged packets
    // LATER(security) Test large amounts of data
    loop {
        // No particular reason for the buffer size, except BufReader uses the same.
        let mut buf = [0; 8192];
        let res = stream.read(&mut buf);
        match res {
            Ok(0) => {
                // The connection has been closed, don't get stuck in this loop.
                // This can happen for example when the server crashes.
                dbg_logf!("Connection closed when reading");
                return true;
            }
            Ok(n) => {
                buffer.extend(&buf[0..n]);
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                return false;
            }
            Err(e) => {
                dbg_logf!("Connection closed when reading - error: {}", e);
                return true;
            }
        }
    }
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
    let msg = bincode::deserialize(&bytes).unwrap();

    Some(msg)
}
