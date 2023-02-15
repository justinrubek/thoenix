use crate::{
    codec::{PktLineCodec, PktLineMessage},
    error::Result,
};

#[derive(Debug)]
pub(crate) enum GitService {
    ReceivePack,
}

#[derive(Debug)]
pub(crate) enum GitMessage {
    ServiceHeader(GitService),
    Data(Vec<u8>),
    Flush,
}

pub(crate) struct GitCodec;

impl tokio_util::codec::Encoder<GitMessage> for GitCodec {
    type Error = crate::error::Error;

    fn encode(&mut self, item: GitMessage, buf: &mut bytes::BytesMut) -> Result<()> {
        match item {
            GitMessage::ServiceHeader(service) => {
                self.encode(service, buf)?;
            }
            GitMessage::Data(data) => {
                let line = PktLineMessage::Data(data);
                PktLineCodec.encode(line, buf)?;
            }
            GitMessage::Flush => {
                buf.extend_from_slice(b"0000");
            }
        }

        Ok(())
    }
}

// each service has a code and comment
impl tokio_util::codec::Encoder<GitService> for GitCodec {
    type Error = crate::error::Error;

    fn encode(&mut self, item: GitService, buf: &mut bytes::BytesMut) -> Result<()> {
        match item {
            GitService::ReceivePack => {
                let line = PktLineMessage::Data(b"# service=git-receive-pack\n".to_vec());
                PktLineCodec.encode(line, buf)?;
            }
        }

        Ok(())
    }
}

impl tokio_util::codec::Decoder for GitCodec {
    type Item = GitMessage;
    type Error = crate::error::Error;

    fn decode(&mut self, buf: &mut bytes::BytesMut) -> Result<Option<Self::Item>> {
        let mut pkt_codec = PktLineCodec;

        let chunk = pkt_codec.decode(buf)?;

        match chunk {
            None => Ok(None),

            Some(PktLineMessage::Data(data)) => match data.as_slice() {
                b"#service git-receive-pack" => {
                    Ok(Some(GitMessage::ServiceHeader(GitService::ReceivePack)))
                }
                data => Ok(Some(GitMessage::Data(data.to_vec()))),
            },

            Some(PktLineMessage::Flush) => Ok(Some(GitMessage::Flush)),
        }
    }
}
