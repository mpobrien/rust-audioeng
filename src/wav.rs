use std::io::{self, Write};

macro_rules! impl_to_bytes {
    ($t:ty) => {
        impl $t {
            fn to_bytes(&self) -> &[u8] {
                unsafe {
                    std::slice::from_raw_parts(
                        self as *const $t as *const u8,
                        std::mem::size_of::<$t>(),
                    )
                }
            }
        }
    };
}

#[repr(C, packed)]
struct RiffHeader {
    chunk_id: [u8; 4], // b"RIFF" literal
    chunk_size: u32,   // file size in bytes, little-endian
    format: [u8; 4],   // b"WAVE" literal
}

#[repr(C, packed)]
struct ChunkHeader {
    id: [u8; 4],
    size: u32,
}

#[repr(u16)]
enum AudioFormat {
    Pcm = 0x0001,
    IeeeFloat = 0x0003,
    ALaw = 0x0006,
    MuLaw = 0x0007,
    Extensible = 0xFFFE,
}

#[repr(C, packed)]
struct FmtChunk {
    audio_format: u16,
    num_channels: u16,
    sample_rate: u32,
    byte_rate: u32,
    block_align: u16,
    bits_per_sample: u16,
}

#[repr(C, packed)]
struct FmtExtension {
    cb_size: u16,
    valid_bits: u16,
    channel_mask: u32,
    sub_format: [u8; 16],
}

#[repr(C, packed)]
struct DataChunkHeader {
    size: u32,
}

impl_to_bytes!(RiffHeader);
impl_to_bytes!(ChunkHeader);
impl_to_bytes!(FmtChunk);
impl_to_bytes!(FmtExtension);
impl_to_bytes!(DataChunkHeader);

/// Accepts streams of samples and writes them to the output
/// stream in .WAV format.
pub fn write_wav(out: &mut impl Write, num_channels: u16, sample_rate: u32, samples: &[f64]) -> io::Result<()> {
    let bits_per_sample: u16 = 32;
    let data_bytes = (samples.len() * 4) as u32;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;

    out.write_all(RiffHeader {
        chunk_id: *b"RIFF",
        chunk_size: 36 + data_bytes,
        format: *b"WAVE",
    }.to_bytes())?;

    out.write_all(ChunkHeader { id: *b"fmt ", size: 16 }.to_bytes())?;

    out.write_all(FmtChunk {
        audio_format: AudioFormat::IeeeFloat as u16,
        num_channels,
        sample_rate,
        byte_rate,
        block_align,
        bits_per_sample,
    }.to_bytes())?;

    out.write_all(ChunkHeader { id: *b"data", size: data_bytes }.to_bytes())?;

    for &s in samples {
        out.write_all(&(s as f32).to_le_bytes())?;
    }

    Ok(())
}
