use crate::error::{Error, Result};
use bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};
use tracing::info;

pub(crate) struct PktLineCodec;

#[derive(Clone, Debug)]
pub(crate) enum PktLineMessage {
    Data(Vec<u8>),
    Flush,
}

impl Encoder<PktLineMessage> for PktLineCodec {
    type Error = Error;

    fn encode(&mut self, item: PktLineMessage, buf: &mut BytesMut) -> Result<()> {
        match item {
            PktLineMessage::Data(data) => {
                let len = data.len() + 4;
                // length in hex
                buf.extend_from_slice(format!("{len:04x}").as_bytes());
                buf.extend_from_slice(data.as_slice());
            }
            PktLineMessage::Flush => {
                buf.extend_from_slice(b"0000");
            }
        }

        Ok(())
    }
}

impl Decoder for PktLineCodec {
    type Item = PktLineMessage;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>> {
        info!(?buf);
        if buf.len() < 4 {
            return Ok(None);
        }

        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&buf[..4]);
        let chunk_len = usize::from_str_radix(
            std::str::from_utf8(&len_bytes).map_err(|_| Error::ParseLengthBytes)?,
            16,
        );

        match chunk_len {
            Ok(0) => {
                buf.advance(4);
                Ok(Some(PktLineMessage::Flush))
            }
            Ok(len) => {
                if buf.len() < len + 4 {
                    return Ok(None);
                }
                buf.advance(4);

                let data = buf.split_to(len - 4).to_vec();
                Ok(Some(PktLineMessage::Data(data)))
            }
            Err(_) => Err(Error::ParseLengthBytes),
        }
    }
}
