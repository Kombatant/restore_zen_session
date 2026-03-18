use anyhow::{Context, Result, bail};

const MAGIC: &[u8; 8] = b"mozLz40\0";
const HEADER_LEN: usize = 12;

pub fn decode(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() < HEADER_LEN {
        bail!("file is too small to be a Mozilla LZ4 file");
    }
    if &bytes[..8] != MAGIC {
        bail!("invalid Mozilla LZ4 header");
    }

    let expected_size = u32::from_le_bytes(
        bytes[8..12]
            .try_into()
            .context("failed to read Mozilla LZ4 size header")?,
    ) as usize;
    let compressed = &bytes[HEADER_LEN..];

    let decompressed = lz4_flex::block::decompress(compressed, expected_size)
        .context("failed to decompress Mozilla LZ4 payload")?;

    if decompressed.len() != expected_size {
        bail!(
            "decompressed size mismatch: expected {} bytes, got {}",
            expected_size,
            decompressed.len()
        );
    }

    Ok(decompressed)
}

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(HEADER_LEN + bytes.len());
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(&lz4_flex::block::compress(bytes));
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_header() {
        let data = b"notlz400";
        assert!(decode(data).is_err());
    }
}
