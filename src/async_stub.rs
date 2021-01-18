use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};

use futures::{
    channel::mpsc::{self as channel, UnboundedSender},
    prelude::*,
    StreamExt,
};

use crate::{MspPacket, MspPacketParseError, MspParser};

pub struct AsyncMspStub {
    tx: UnboundedSender<Request>,
}

struct Request {
    message: MspPacket,
    backchannel: UnboundedSender<Result<MspPacket, MspPacketParseError>>,
}

struct Pending {
    cmd: u16,
    backchannel: UnboundedSender<Result<MspPacket, MspPacketParseError>>,
    ts: Instant,
}

impl AsyncMspStub {
    pub fn new<R: AsyncRead + Unpin + Send, W: AsyncWrite + Unpin + Send>(
        mut reader: R,
        mut writer: W,
        timeout: Duration,
    ) -> io::Result<(Self, impl Future<Output = impl Send> + Send)> {
        let (tx, mut rx) = channel::unbounded();
        let f = async move {
            let mut parser = MspParser::new();
            let mut buf = [0u8; 1];
            let mut pending: VecDeque<Pending> = VecDeque::with_capacity(16);

            loop {
                futures::select! {
                    // maybe a new response?
                    _ = reader.read_exact(&mut buf[..]).fuse() =>
                           match parser.parse(buf[0]){
                               Ok(Some(response))=>{
                                while let Some(p) = pending.front_mut(){
                                    if p.ts.elapsed()> timeout {
                                        p.backchannel.send(Err(MspPacketParseError::TimedOut)).await.unwrap();  // TODO convert to an err
                                        pending.pop_front();
                                    }else if p.cmd == response.cmd{
                                        p.backchannel.send(Ok(response)).await.unwrap(); // TODO convert to an err
                                        pending.pop_front();
                                        break;
                                    }
                               }
                               }
                               _=>{
                                   // TODO what now?
                               }
                           },
                       // a new request
                    new_request = rx.next() =>{

                        let Request {message, backchannel } = new_request.unwrap(); // TODO convert to an err
                            let mut buf = Vec::with_capacity(message.packet_size_bytes_v2());
                            message.serialize_v2(&mut buf).unwrap(); // TODO convert to an err
                            writer.write(&buf).await.unwrap(); // TODO convert to an err
                            buf.clear();
                            pending.push_back(Pending{cmd: message.cmd,  backchannel, ts: Instant::now()})
                    }
                }
            }
        };
        Ok((AsyncMspStub { tx }, f))
    }

    pub async fn request<M: Into<MspPacket>>(
        &self,
        request: M,
    ) -> Result<MspPacket, MspPacketParseError> {
        let (backchannel, mut response) = channel::unbounded();

        self.tx
            .clone()
            .send(Request {
                message: request.into(),
                backchannel,
            })
            .await;

        response.next().await.unwrap() //TODO convert to an err
    }
}
