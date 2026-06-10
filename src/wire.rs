use crate::Result;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncReadExt};

const MAX_FRAME_LENGTH: u32 = 64 * 1024 * 1024;

pub(crate) fn serialize_payload(data: &impl serde::Serialize) -> Result<Vec<u8>> {
    let payload = postcard::to_allocvec(data)?;
    if payload.len() > MAX_FRAME_LENGTH as usize {
        return Err("frame length exceeds maximum".into());
    }
    Ok(payload)
}

pub(crate) fn frame_payload(payload: &[u8]) -> Vec<u8> {
    let length = payload.len() as u32;
    let mut frame = length.to_be_bytes().to_vec();
    frame.extend_from_slice(payload);
    frame
}

pub(crate) async fn read_frame<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
) -> Result<T> {
    let mut length_buffer = [0_u8; 4];
    reader.read_exact(&mut length_buffer).await?;
    let length = u32::from_be_bytes(length_buffer);
    if length > MAX_FRAME_LENGTH {
        return Err("frame length exceeds maximum".into());
    }
    let mut frame_bytes = vec![0_u8; length as usize];
    reader.read_exact(&mut frame_bytes).await?;
    Ok(postcard::from_bytes(&frame_bytes)?)
}
