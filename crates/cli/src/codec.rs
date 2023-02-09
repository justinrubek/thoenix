#![allow(dead_code)]
use bytes::Buf;
use std::io::Read;
use tokio_util::codec::{Decoder, Encoder};

use crate::error::{AppError, AppResult};

/// git protocol encoder/decoder
struct ChunkCodec;

#[derive(Clone, Debug)]
pub(crate) struct GitCodec {
    pack_data: bool,
}

impl GitCodec {
    pub(crate) fn new() -> Self {
        Self { pack_data: false }
    }
}

const CHUNK_LENGTH_BYTES: usize = 4;
const CONT_MASK: u8 = 0b1000_0000;
const TYPE_MASK: u8 = 0b0111_0000;
const SIZE_4_MASK: u8 = 0b0000_1111;
const SIZE_7_MASK: u8 = 0b0111_1111;

#[derive(Clone, Debug)]
pub(crate) struct PackEntry {
    pub type_id: u8,
    pub size: u32,
    pub header_len: usize,
}

#[derive(Clone, Debug)]
struct PackHunk {
    pub cont: bool,
    pub size: u32,
    pub type_id: Option<u8>,
    pub offset_size: usize,
}

impl PackHunk {
    fn decode_with_type(data: &[u8]) -> Self {
        let cont = data[0] & CONT_MASK != 0;
        let type_id = (data[0] & TYPE_MASK) >> 4;
        let size = (data[0] & SIZE_4_MASK) as u32;

        Self {
            cont,
            size,
            type_id: Some(type_id),
            offset_size: 4,
        }
    }

    fn decode_without_type(data: &[u8]) -> Self {
        let cont = data[0] & CONT_MASK != 0;
        let size = (data[0] & SIZE_7_MASK) as u32;

        Self {
            cont,
            size,
            type_id: None,
            offset_size: 7,
        }
    }
}

#[derive(Clone, Debug)]
struct EntryData {
    compressed: bytes::Bytes,
    uncompressed: bytes::Bytes,
}

impl EntryData {
    fn try_decode(data: &[u8]) -> AppResult<Self> {
        let mut decoder = flate2::read::ZlibDecoder::new(data);
        let mut uncompressed = Vec::new();
        decoder.read_to_end(&mut uncompressed)?;

        let compressed = bytes::Bytes::copy_from_slice(data);

        Ok(Self {
            compressed,
            uncompressed: uncompressed.into(),
        })
    }

    fn bruteforce_decode(data: &[u8]) -> AppResult<Option<Self>> {
        // Since we don't have an index for this pack, we need to try to decode the entry
        // We start with a single byte and brute-force until it succeeds
        tracing::info!("bruteforcing pack entry");

        let mut num_bytes = 1;
        while num_bytes < data.len() {
            tracing::info!("trying {} bytes", num_bytes);
            let data = Self::try_decode(&data[..num_bytes]);

            match data {
                Ok(data) => return Ok(Some(data)),
                Err(_) => {
                    num_bytes += 1;
                    continue;
                }
            }
        }

        // We tried all possible sizes and none worked, so wait for more data
        tracing::info!("no bytes worked, waiting for more data");
        Ok(None)
    }
}

/// Parser the pack's object entry header
#[inline]
fn parse_entry_header(data: &[u8]) -> PackEntry {
    let mut parsed = PackHunk::decode_with_type(data);

    let type_id = parsed.type_id.unwrap();
    let mut size = parsed.size;
    let mut offset = parsed.offset_size;

    while parsed.cont {
        parsed = PackHunk::decode_without_type(&data[offset..]);
        size |= parsed.size << 7;
        offset += parsed.offset_size;
    }

    PackEntry {
        type_id,
        size,
        header_len: offset,
    }
}

#[derive(Debug)]
pub(crate) struct PackFile {
    pub(crate) entries: Vec<PackEntry>,
}

impl PackFile {
    pub(crate) fn new(entries: Vec<PackEntry>) -> Self {
        Self { entries }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum GitMessage {
    Data(Vec<u8>),
    PackData(Vec<u8>),
}

impl Decoder for GitCodec {
    type Item = GitMessage;
    type Error = AppError;

    fn decode(&mut self, buffer: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buffer.len() < CHUNK_LENGTH_BYTES {
            return Ok(None);
        }
        // read the length of the chunk
        let mut len_bytes = [0u8; CHUNK_LENGTH_BYTES];
        len_bytes.copy_from_slice(&buffer[..CHUNK_LENGTH_BYTES]);

        // First, check if len_bytes is PACK
        if len_bytes[0..4] == b"PACK"[..] {
            self.pack_data = true;
        }
        if self.pack_data {
            // take the entire buffer and return it as PackData
            let data = buffer.split_to(buffer.len());

            Ok(Some(GitMessage::PackData(data.to_vec())))
        } else {
            let chunk_len = usize::from_str_radix(
                std::str::from_utf8(&len_bytes).map_err(|_| AppError::ParseLengthBytes)?,
                16,
            )
            .map_err(|_| AppError::InvalidChunkLength)?;

            tracing::info!(?chunk_len, "decode");

            match chunk_len {
                0 => {
                    tracing::info!("got 0 chunk length");
                }
                1 | 2 => {
                    tracing::info!("got 1 or 2 chunk length");
                }
                _ => {
                    tracing::info!("got > 2 chunk length");
                }
            }

            if chunk_len == 0 {
                // TODO: end of stream?
                // return Ok(Some(vec![]));
                return Ok(Some(GitMessage::Data(vec![])));
            }

            // the length includes the length bytes themselves, so subtract them
            let chunk_len = chunk_len
                .checked_sub(CHUNK_LENGTH_BYTES)
                .ok_or_else(|| AppError::Anyhow(anyhow::anyhow!("invalid chunk length")))?;

            // check if the entire chunk is in the buffer
            if buffer.len() < chunk_len + CHUNK_LENGTH_BYTES {
                return Ok(None);
            }

            // skip the length, get the chunk
            let chunk: Vec<u8> = buffer
                .iter()
                .skip(CHUNK_LENGTH_BYTES)
                .take(chunk_len)
                .copied()
                .collect();
            // remove the chunk from the buffer
            buffer.advance(chunk_len + CHUNK_LENGTH_BYTES);

            Ok(Some(GitMessage::Data(chunk)))
        }
    }
}

impl Encoder<GitMessage> for GitCodec {
    type Error = AppError;

    fn encode(&mut self, item: GitMessage, buf: &mut bytes::BytesMut) -> Result<(), Self::Error> {
        match item {
            GitMessage::Data(data) => {
                if data.is_empty() {
                    // a zero-length chunk is the end of the stream, but we need to send 0000
                    buf.extend_from_slice(b"0000");
                } else {
                    let chunk_len = data.len() + CHUNK_LENGTH_BYTES;
                    let chunk_len_hex = format!("{chunk_len:04x}");
                    buf.extend_from_slice(chunk_len_hex.as_bytes());
                    buf.extend_from_slice(&data);
                }
            }
            GitMessage::PackData(data) => {
                buf.extend_from_slice(&data);
            }
        }

        Ok(())
    }
}
