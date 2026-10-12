//! Deterministic, paced PCM input for the editor. This is a simulated device,
//! registered directly on the Edge Node, like a hardware audio source.
use async_trait::async_trait;
use operit_edge_contract::audio::{AudioInputDevice, AudioInputFormat};
use operit_node_edge::audio::{EdgeAudioSession, EdgeAudioSource};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use base64::{engine::general_purpose::STANDARD, Engine};
use tokio::sync::mpsc;

#[derive(Clone, Copy)]
struct Settings {
    blocks: u32,
    fail: bool,
}
struct State {
    settings: Settings,
    browser: Option<(String, mpsc::Sender<Vec<u8>>)>,
    active: bool,
    blocks: u32,
    bytes: u32,
    error: Option<String>,
}
#[derive(Clone)]
pub struct SimulatorMicrophone {
    state: Arc<Mutex<State>>,
}
impl SimulatorMicrophone {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                settings: Settings {
                    blocks: 50,
                    fail: false,
                },
                browser: None,
                active: false,
                blocks: 0,
                bytes: 0,
                error: None,
            })),
        }
    }
    pub fn configure(&self, input: &serde_json::Value) -> Result<serde_json::Value, String> {
        let duration = input["durationMs"]
            .as_u64()
            .ok_or("Missing audio durationMs")?;
        let fail = input["fail"].as_bool().ok_or("Missing audio fail flag")?;
        if duration < 20 || duration > 10000 || duration % 20 != 0 {
            return Err("Audio duration must be 20..10000 ms, in 20 ms steps".into());
        }
        let mut state = self.state.lock().unwrap();
        if state.active {
            return Err("Stop the active capture before changing its input".into());
        }
        state.settings = Settings {
            blocks: (duration / 20) as u32,
            fail,
        };
        drop(state);
        Ok(self.snapshot())
    }
    pub fn pushBrowser(&self, input: &serde_json::Value) -> Result<serde_json::Value, String> {
        let mut state = self.state.lock().unwrap();
        let (capture, sender) = state.browser.as_ref().ok_or("Browser microphone is not recording")?;
        if input["captureId"].as_str() != Some(capture) { return Err("Browser capture identity changed".into()); }
        if let Some(error) = input["error"].as_str() {
            state.error = Some(error.chars().take(120).collect());
            state.browser.take();
            return Ok(serde_json::json!({"ok": true}));
        }
        let pcm = STANDARD.decode(input["pcm"].as_str().ok_or("Missing browser PCM")?).map_err(|e| e.to_string())?;
        AudioInputFormat::default().validateChunk(&pcm)?;
        if sender.try_send(pcm).is_err() {
            state.error = Some("Browser microphone overrun".into());
            state.browser.take();
            return Err("Browser microphone overrun".into());
        }
        Ok(serde_json::json!({"ok": true}))
    }
    pub fn snapshot(&self) -> serde_json::Value {
        let state = self.state.lock().unwrap();
        serde_json::json!({"inputId": if state.browser.is_some() { "browser-mic" } else { "sim-pcm" },
            "captureId": state.browser.as_ref().map(|(id, _)| id), "durationMs": state.settings.blocks * 20,
            "fail": state.settings.fail, "active": state.active, "blocks": state.blocks,
            "byteLength": state.bytes, "error": state.error,
            "format": AudioInputFormat::default()})
    }
}
struct Recording {
    state: Arc<Mutex<State>>,
    settings: Settings,
    offset: u32,
    started: Instant,
}
#[async_trait]
impl EdgeAudioSource for SimulatorMicrophone {
    fn listInputs(&self) -> Result<Vec<AudioInputDevice>, String> {
        Ok(vec![AudioInputDevice {
            inputId: "sim-pcm".into(),
            name: "Simulated PCM microphone (440 Hz)".into(),
            formats: vec![AudioInputFormat::default()],
        }, AudioInputDevice { inputId: "browser-mic".into(), name: "Browser microphone".into(), formats: vec![AudioInputFormat::default()] }])
    }
    async fn openInput(
        &self,
        input: &str,
        format: &AudioInputFormat,
    ) -> Result<Box<dyn EdgeAudioSession>, String> {
        if !matches!(input, "sim-pcm" | "browser-mic") || format != &AudioInputFormat::default() {
            return Err("Unsupported simulator audio input or format".into());
        }
        let mut state = self.state.lock().unwrap();
        if state.active {
            return Err("Simulator microphone already active".into());
        }
        state.active = true;
        state.blocks = 0;
        state.bytes = 0;
        state.error = None;
        if input == "browser-mic" {
            let (sender, receiver) = mpsc::channel(8);
            let capture = uuid::Uuid::new_v4().to_string();
            state.browser = Some((capture.clone(), sender));
            return Ok(Box::new(BrowserRecording { state: self.state.clone(), capture, receiver }));
        }
        Ok(Box::new(Recording {
            state: self.state.clone(),
            settings: state.settings,
            offset: 0,
            started: Instant::now(),
        }))
    }
}
#[async_trait]
impl EdgeAudioSession for Recording {
    async fn read(&mut self) -> Result<Option<Vec<u8>>, String> {
        if self.offset == self.settings.blocks {
            if self.settings.fail {
                self.state.lock().unwrap().error = Some("Simulated microphone overrun".into());
                return Err("Simulated microphone overrun".into());
            }
            return Ok(None);
        }
        // 320 samples = 20 ms at 16 kHz. Absolute deadlines prevent cumulative drift.
        let deadline = self.started + Duration::from_millis((self.offset as u64 + 1) * 20);
        tokio::time::sleep_until(deadline.into()).await;
        let mut bytes = Vec::with_capacity(640);
        for sample in self.offset * 320..(self.offset + 1) * 320 {
            let value =
                ((sample as f64 * 440.0 * std::f64::consts::TAU / 16000.0).sin() * 8000.0) as i16;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        self.offset += 1;
        let mut state = self.state.lock().unwrap();
        state.blocks += 1;
        state.bytes += bytes.len() as u32;
        Ok(Some(bytes))
    }
}
impl Drop for Recording {
    fn drop(&mut self) {
        self.state.lock().unwrap().active = false;
    }
}

struct BrowserRecording {
    state: Arc<Mutex<State>>,
    capture: String,
    receiver: mpsc::Receiver<Vec<u8>>,
}
#[async_trait]
impl EdgeAudioSession for BrowserRecording {
    async fn read(&mut self) -> Result<Option<Vec<u8>>, String> {
        let bytes = tokio::time::timeout(Duration::from_secs(10), self.receiver.recv()).await
            .map_err(|_| "Browser microphone timed out".to_string())?;
        let mut state = self.state.lock().unwrap();
        if let Some(error) = state.error.clone() { return Err(error); }
        if let Some(bytes) = &bytes { state.blocks += 1; state.bytes += bytes.len() as u32; }
        Ok(bytes)
    }
    fn readBuffered(&mut self) -> Result<Option<Vec<u8>>, String> {
        let mut state = self.state.lock().unwrap();
        if state.browser.as_ref().is_some_and(|(id, _)| id == &self.capture) { state.browser.take(); }
        if let Some(error) = state.error.clone() { return Err(error); }
        let bytes = self.receiver.try_recv().ok();
        if let Some(bytes) = &bytes { state.blocks += 1; state.bytes += bytes.len() as u32; }
        Ok(bytes)
    }
}
impl Drop for BrowserRecording {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap();
        if state.browser.as_ref().is_some_and(|(id, _)| id == &self.capture) { state.browser.take(); }
        state.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn browser_finish_drains_accepted_pcm_and_rejects_new_samples() {
        let mic = SimulatorMicrophone::new();
        let mut input = mic.openInput("browser-mic", &AudioInputFormat::default()).await.unwrap();
        let id = mic.snapshot()["captureId"].as_str().unwrap().to_string();
        for value in 1..=8u8 {
            mic.pushBrowser(&serde_json::json!({"captureId":id,"pcm":STANDARD.encode(vec![value;640])})).unwrap();
        }
        for value in 1..=8u8 { assert_eq!(input.readBuffered().unwrap().unwrap(), vec![value;640]); }
        assert!(input.readBuffered().unwrap().is_none());
        assert!(mic.pushBrowser(&serde_json::json!({"captureId":id,"pcm":STANDARD.encode(vec![9;640])})).is_err());
        assert_eq!(mic.snapshot()["byteLength"], 5120);
        drop(input);
        assert_eq!(mic.snapshot()["active"], false);
    }
    #[tokio::test]
    async fn browser_source_rejects_stale_capture_and_reports_overrun_without_dropping_samples() {
        let mic = SimulatorMicrophone::new();
        let mut input = mic.openInput("browser-mic", &AudioInputFormat::default()).await.unwrap();
        let id = mic.snapshot()["captureId"].as_str().unwrap().to_string();
        let block = STANDARD.encode(vec![1u8; 640]);
        assert!(mic.pushBrowser(&serde_json::json!({"captureId":"old", "pcm":block})).is_err());
        mic.pushBrowser(&serde_json::json!({"captureId":id, "pcm":block})).unwrap();
        assert_eq!(input.read().await.unwrap().unwrap(), vec![1u8;640]);
        for _ in 0..8 { mic.pushBrowser(&serde_json::json!({"captureId":id,"pcm":block})).unwrap(); }
        assert!(mic.pushBrowser(&serde_json::json!({"captureId":id,"pcm":block})).is_err());
        assert!(input.read().await.unwrap_err().contains("overrun"));
        drop(input);
        assert_eq!(mic.snapshot()["active"],false);
        let fresh = mic.openInput("browser-mic", &AudioInputFormat::default()).await.unwrap();
        assert_ne!(mic.snapshot()["captureId"],id);
        drop(fresh);
    }
    #[tokio::test]
    async fn paced_input_reports_eof_overrun_and_releases_on_drop() {
        let mic = SimulatorMicrophone::new();
        mic.configure(&serde_json::json!({"durationMs": 20, "fail": false}))
            .unwrap();
        let mut input = mic
            .openInput("sim-pcm", &AudioInputFormat::default())
            .await
            .unwrap();
        assert!(mic
            .configure(&serde_json::json!({"durationMs": 20, "fail": true}))
            .is_err());
        let bytes = input.read().await.unwrap().unwrap();
        assert_eq!(bytes.len(), 640);
        assert_eq!(&bytes[..4], &[0, 0, 95, 5]);
        assert!(input.read().await.unwrap().is_none());
        drop(input);
        assert_eq!(mic.snapshot()["active"], false);
        mic.configure(&serde_json::json!({"durationMs": 20, "fail": true}))
            .unwrap();
        let mut input = mic
            .openInput("sim-pcm", &AudioInputFormat::default())
            .await
            .unwrap();
        input.read().await.unwrap();
        assert!(input
            .read()
            .await
            .unwrap_err()
            .to_string()
            .contains("overrun"));
        drop(input);
        assert_eq!(mic.snapshot()["active"], false);
        assert!(mic
            .configure(&serde_json::json!({"durationMs": 21, "fail": false}))
            .is_err());
    }
}
