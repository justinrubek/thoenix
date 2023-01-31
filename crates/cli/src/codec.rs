use bytes::Buf;
use tokio_util::codec::{Decoder, Encoder};

use crate::error::AppError;

/// git protocol encoder/decoder
struct ChunkCodec;

const CHUNK_LENGTH_BYTES: usize = 4;

fn hex_char_value(byte: u8) -> Option<u8> {
    if byte == 0 {
        return Some(0);
    }
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

impl Decoder for ChunkCodec {
    type Item = Vec<u8>;
    type Error = AppError;

    fn decode(&mut self, buf: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.len() < CHUNK_LENGTH_BYTES {
            return Ok(None);
        }
        // read the length of the chunk
        let chunk_len = (buf[0..CHUNK_LENGTH_BYTES])
            .iter()
            .try_fold(0, |value, &byte| {
                let char_value = hex_char_value(byte);
                let char_value = char_value?;
                Some(value << 4 | char_value as usize)
            })
            .ok_or_else(|| AppError::Anyhow(anyhow::anyhow!("invalid chunk length")))?;
        tracing::info!(?chunk_len, "decode");

        if chunk_len == 0 {
            // TODO: end of stream?
            return Ok(Some(vec![]));
        }

        // the length includes the length bytes themselves, so subtract them
        let chunk_len = chunk_len
            .checked_sub(CHUNK_LENGTH_BYTES)
            .ok_or_else(|| AppError::Anyhow(anyhow::anyhow!("invalid chunk length")))?;

        // check if the entire chunk is in the buffer
        if buf.len() < chunk_len + CHUNK_LENGTH_BYTES {
            return Ok(None);
        }

        // skip the length, get the chunk
        let chunk: Vec<u8> = buf
            .iter()
            .skip(CHUNK_LENGTH_BYTES)
            .take(chunk_len)
            .copied()
            .collect();
        // remove the chunk from the buffer
        buf.advance(chunk_len + CHUNK_LENGTH_BYTES);

        Ok(Some(chunk))
    }
}

impl Encoder<Vec<u8>> for ChunkCodec {
    type Error = AppError;

    fn encode(&mut self, item: Vec<u8>, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
        if item.is_empty() {
            // a zero-length chunk is the end of the stream, but we need to send 0000
            dst.extend_from_slice(b"0000");
        } else {
            let chunk_len = item.len() + CHUNK_LENGTH_BYTES;
            let chunk_len_hex = format!("{chunk_len:04x}");
            dst.extend_from_slice(chunk_len_hex.as_bytes());
            dst.extend_from_slice(&item);
        }

        Ok(())
    }
}

struct TextChunkCodec;

impl Decoder for TextChunkCodec {
    type Item = String;
    type Error = AppError;

    fn decode(&mut self, buf: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let chunk = ChunkCodec.decode(buf)?;
        if let Some(chunk) = chunk {
            let mut chunk = String::from_utf8(chunk)?;

            // Remove any trailing newlines as they are not needed
            if chunk.ends_with('\n') {
                chunk.pop();
            }

            Ok(Some(chunk))
        } else {
            Ok(None)
        }
    }
}

impl Encoder<String> for TextChunkCodec {
    type Error = AppError;

    fn encode(&mut self, item: String, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
        ChunkCodec.encode(item.into_bytes(), dst)
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::{ChunkCodec, TextChunkCodec};
    use crate::error::{AppError, AppResult};
    use tokio_util::codec::{Decoder, Encoder};

    #[tokio::test]
    async fn encode_strings() {
        let mut codec = TextChunkCodec;
        let mut buf = bytes::BytesMut::new();
        let chunk_contents = "cded0bbfe0b0a2c44a823d7bca226555f98200cd refs/heads/main\0report-status report-status-v2 delete-refs side-band-64k quiet atomic ofs-delta object-format=sha1 agent=git/2.38.1\n";
        codec.encode(chunk_contents.to_string(), &mut buf).unwrap();

        let mut expected = bytes::BytesMut::new();
        let expected_string = "00b1cded0bbfe0b0a2c44a823d7bca226555f98200cd refs/heads/main\0report-status report-status-v2 delete-refs side-band-64k quiet atomic ofs-delta object-format=sha1 agent=git/2.38.1\n";
        expected.extend_from_slice(expected_string.as_bytes());

        assert_eq!(buf, expected);
    }

    #[tokio::test]
    async fn decode_strings() {
        let mut codec = TextChunkCodec;
        let mut buf = bytes::BytesMut::new();
        let chunk_contents = "cded0bbfe0b0a2c44a823d7bca226555f98200cd refs/heads/main\0report-status report-status-v2 delete-refs side-band-64k quiet atomic ofs-delta object-format=sha1 agent=git/2.38.1\n";
        codec.encode(chunk_contents.to_string(), &mut buf).unwrap();

        let decoded = codec.decode(&mut buf).unwrap().unwrap();

        // Our decoder removes any trailing newlines, so we need to do the same
        let mut expected = chunk_contents.to_string();
        expected.pop();

        assert_eq!(decoded, expected);
    }

    #[tokio::test]
    async fn encode_empty_chunk() {
        let mut codec = TextChunkCodec;
        let mut buf = bytes::BytesMut::new();
        codec.encode("".to_string(), &mut buf).unwrap();

        let mut expected = bytes::BytesMut::new();
        expected.extend_from_slice("0000".as_bytes());

        assert_eq!(buf, expected);
    }

    #[tokio::test]
    async fn decode_empty_chunk() -> AppResult<()> {
        let mut codec = TextChunkCodec;
        let mut buf = bytes::BytesMut::new();
        codec.encode("".to_string(), &mut buf).unwrap();

        let decoded = codec.decode(&mut buf)?.ok_or_else(|| {
            AppError::Anyhow(anyhow::anyhow!("failed to properly handle empty chunk"))
        })?;

        assert_eq!(decoded, "");

        Ok(())
    }
}
