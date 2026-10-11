//! Audio uses ordinary Link Call controls and binary Link Push items.
use serde::{Deserialize, Serialize};

pub const AUDIO_MAX_CHUNK_BYTES: usize = 4096;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioInputFormat {
    pub encoding: String,
    pub sampleRateHz: u32,
    pub channels: u8,
}
impl Default for AudioInputFormat {
    fn default() -> Self {
        Self {
            encoding: "pcm_s16le".into(),
            sampleRateHz: 16000,
            channels: 1,
        }
    }
}
impl AudioInputFormat {
    pub fn validate(&self) -> Result<(), String> {
        if self.encoding != "pcm_s16le"
            || !matches!(self.sampleRateHz, 8000 | 16000 | 24000 | 48000)
            || !matches!(self.channels, 1 | 2)
        {
            return Err("Unsupported PCM format".into());
        }
        Ok(())
    }
    pub fn validateChunk(&self, bytes: &[u8]) -> Result<(), String> {
        self.validate()?;
        if bytes.is_empty()
            || bytes.len() > AUDIO_MAX_CHUNK_BYTES
            || bytes.len() % (2 * self.channels as usize) != 0
        {
            return Err("PCM chunk must contain complete samples and 1..4096 bytes".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioInputDevice {
    pub inputId: String,
    pub name: String,
    pub formats: Vec<AudioInputFormat>,
}

pub const EDGE_AUDIO_TARGET: &str = "edge.audio";
pub const CORE_AUDIO_INGRESS_TARGET: &str = "edge.audio.ingress";
pub const CORE_AUDIO_INGRESS_METHOD: &str = "upload";
pub const AUDIO_MAX_DURATION_MS: u32 = 300_000;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartAudioInput {
    pub streamId: String,
    pub receiverNodeId: String,
    pub inputId: String,
    pub format: AudioInputFormat,
    pub maxDurationMs: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioIngressOpen {
    pub streamId: String,
    pub format: AudioInputFormat,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopAudioInput {
    pub streamId: String,
}
