use std::any::{Any, TypeId};
use std::collections::VecDeque;
use std::io;
use std::io::{Read, Write};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{
    self,
    channel::mpsc::{self as channel, UnboundedSender},
    future::Either,
    prelude::*,
    StreamExt,
};

use crate::{MspPacket, MspParser, MspPayload};

pub trait MspConnection: Read + Write {}
impl<T: Read + Write> MspConnection for T {}

struct AsyncMspConnection {
    /*
pending: Mutex<VecDeque<Request>>,
msp_version: MspVersion,
//reader_: Pin<Arc<Mutex<R>>>,
responses: Pin<Arc<Mutex<S>>>,
writer: Pin<Arc<Mutex<W>>>,

tx: Sender<Request>,
*/

/*
rx: Receiver<Request>,
ttl: Duration,
*/}

struct Request {
    packet: (TypeId, Vec<u8>),
    backchannel: UnboundedSender<Box<dyn Any>>,
    t0: Instant,
}

pub enum MspVersion {
    V1,
    V2,
}

/*
impl AsyncMspConnection {
    pub fn new<R,W> ( reader: R, writer: W, msp_version: MspVersion) -> (Self, impl Future<Output =
                                                                         impl Send> + Send)

where
        R: AsyncRead + std::marker::Unpin,
        W: AsyncWrite + std::marker::Unpin,

                {
        let (tx, rx) = channel::unbounded();
        let ttl = Duration::from_millis(100);

        let f = async move {
            let mut parser = MspParser::new();
            let mut buf_reader = futures::io::BufReader::new(reader);

            let requests = rx.map(Either::Left);

            let responses = buf_reader
                .bytes()
                .filter_map(move |maybe_byte| {
                    match maybe_byte {
                        Ok(byte) => parser // .lock().await
                            .parse(byte)
                            .map_err(|e| {
                                io::Error::new(io::ErrorKind::InvalidData, format!("{:?}", e))
                            })
                            .transpose(),
                        Err(e) => Some(Err(e)),
                    }
                })
                .map(Either::Right); // TODO is this valid?

            let combined = stream::select(requests, responses);

            loop {
                match combined.next().await.unwrap() {
                    Either::Left(req) if req.t0.elapsed() <= ttl => {
                        match msp_version {
                            V2 => {
                                let mut buf = Vec::with_capacity(req.packet.packet_size_bytes());
                                let p = req.packet.serialize(&mut buf);
                                writer.write(&buf).await;
                            }
                            V2 => {
                                let mut buf = Vec::with_capacity(req.packet.packet_size_bytes_v2());
                                let p = req.packet.serialize_v2(&mut buf);
                                writer.write(&buf).await;
                            }
                        }
                        req.push_back(req);
                    }
                    Either::Right(Ok(packet)) => {}
                    _ => {}
                }
            }
        };

        (AsyncMspConnection { rx }, f)
    }

    pub fn request<T: MspPayload>(request: T) -> T {
        let typeid = TypeId::of::<T>();
        let mut buf = Vec::new();
        todo!();
    }
}
*/
