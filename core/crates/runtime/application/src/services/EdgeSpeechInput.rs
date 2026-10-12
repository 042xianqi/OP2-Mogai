//! Core-owned speech input. Device PCM uses the existing audio registry/Link Push;
//! only the final text returns to the Edge UI. No model or config lives on Edge.
#![allow(non_snake_case)]
use super::SttRecognitionService::SttRecognitionService;
use base64::{engine::general_purpose::STANDARD, Engine};
use operit_host_api::HostManager::{HostManager, defaultHostRuntimeTaskSchedulerHost};
use operit_link::{toCoreValue, CoreCallRequest};
use operit_tools::runtime_support::{edge_audio::EdgeAudioRegistry, ToolRuntimeSupport};
use std::{collections::HashMap, sync::{Arc, Mutex}, time::{Duration, Instant}};
use tokio::sync::watch;

/// User-facing failures travel as bounded state, rather than framework errors
/// carrying desktop backtraces larger than the existing 8 KiB Edge frame limit.
pub(crate) fn reply(result: Result<serde_json::Value, String>) -> serde_json::Value {
    match result {
        Ok(value) => value,
        Err(error) => serde_json::json!({"error": error.chars().take(240).collect::<String>()}),
    }
}

const MAX_RECORD_MS: u32 = 60_000;
const MAX_PCM_BYTES: usize = 16_000 * 2 * 60;
const LEASE: Duration = Duration::from_secs(15);
struct Recording {
    chat: String,
    node: String,
    registry: Arc<EdgeAudioRegistry>,
    cancel: watch::Sender<bool>,
    state: Mutex<State>,
}
struct State {
    phase: &'static str,
    text: Option<String>,
    error: Option<String>,
    finishRequested: bool,
    polled: Instant,
}
#[derive(Clone, Default)]
pub(crate) struct EdgeSpeechInput {
    recordings: Arc<Mutex<HashMap<String, Arc<Recording>>>>,
}
impl EdgeSpeechInput {
    pub(crate) async fn begin(&self, chat: String, node: String, input: Option<String>,
        runtime: Arc<dyn ToolRuntimeSupport>, context: HostManager) -> Result<String, String> {
        let stt = SttRecognitionService::getInstance(&context);
        stt.validateCurrentConfiguration()?;
        let format = operit_edge_contract::audio::AudioInputFormat::default();
        let registry = runtime.edgeAudioRegistry()?;
        let (cancel, cancelled) = watch::channel(false);
        let recording = Arc::new(Recording { chat, node: node.clone(), registry: registry.clone(), cancel,
            state: Mutex::new(State { phase: "starting", text: None, error: None, finishRequested: false, polled: Instant::now() }) });
        let id = {
            let mut recordings = self.recordings.lock().unwrap();
            recordings.retain(|id, value| {
                let state = value.state.lock().unwrap();
                if state.polled.elapsed() < Duration::from_secs(120) { return true; }
                value.cancel.send_replace(true);
                value.registry.close(id);
                false
            });
            if recordings.values().any(|r| r.node == node && r.state.lock().unwrap().phase != "done") {
                return Err("该设备已有录音正在处理".into());
            }
            if recordings.len() >= 8 { return Err("Speech input capacity exceeded".into()); }
            let id = registry.register(node.clone(), format.clone(), MAX_RECORD_MS)?;
            recordings.insert(id.clone(), recording.clone());
            id
        };
        // The registry reservation precedes device capture. A separate worker
        // owns reverse calls and continuous consumption of the bounded queue.
        let taskId = id.clone();
        let taskRuntime = runtime.clone();
        let taskRecording = recording.clone();
        let scheduled = defaultHostRuntimeTaskSchedulerHost().scheduleHostRuntimeAsyncTask("edge-speech",
            Box::new(move || Box::pin(async move {
                collect(taskId, taskRecording, taskRuntime, stt, input, cancelled).await;
            })));
        if let Err(error) = scheduled {
            registry.close(&id);
            self.recordings.lock().unwrap().remove(&id);
            return Err(error.to_string());
        }
        // Reply before opening the reverse capture. Waiting for startInput here
        // would block this carrier's ordered ingress behind beginEdgeSpeech.
        Ok(id)
    }

    fn recording(&self, chat: &str, node: &str, id: &str) -> Result<Arc<Recording>, String> {
        let recording = self.recordings.lock().unwrap().get(id).cloned().ok_or("录音已结束或已过期")?;
        if recording.chat != chat || recording.node != node { return Err("录音不属于当前设备或对话".into()); }
        recording.state.lock().unwrap().polled = Instant::now();
        Ok(recording)
    }
    pub(crate) fn poll(&self, chat: &str, node: &str, id: &str) -> Result<serde_json::Value, String> {
        let recording = self.recording(chat, node, id)?;
        let state = recording.state.lock().unwrap();
        Ok(serde_json::json!({"phase": state.phase, "text": state.text, "error": state.error}))
    }
    pub(crate) async fn finish(&self, chat: &str, node: &str, id: &str, runtime: Arc<dyn ToolRuntimeSupport>) -> Result<(), String> {
        let recording = self.recording(chat, node, id)?;
        {
            let mut state = recording.state.lock().unwrap();
            if state.phase == "starting" { state.finishRequested = true; return Ok(()); }
            if state.phase != "recording" { return Ok(()); }
        }
        audioCall(&runtime, node, "finishInput", serde_json::json!({"streamId": id})).await?;
        Ok(())
    }
    pub(crate) async fn cancel(&self, chat: &str, node: &str, id: &str, runtime: Arc<dyn ToolRuntimeSupport>) -> Result<(), String> {
        let recording = self.recording(chat, node, id)?;
        recording.cancel.send_replace(true);
        recording.registry.close(id);
        self.recordings.lock().unwrap().remove(id);
        audioCall(&runtime, node, "stopInput", serde_json::json!({"streamId": id})).await?;
        Ok(())
    }
}
async fn audioCall(runtime: &Arc<dyn ToolRuntimeSupport>, node: &str, method: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
    runtime.executeEdgeAudio(node.into(), CoreCallRequest::new(
        operit_link::nextCoreRouteRequestId(method), "edge.audio", method, toCoreValue(args).map_err(|e| e.to_string())?
    )).await
}
async fn collect(id: String, recording: Arc<Recording>, runtime: Arc<dyn ToolRuntimeSupport>, stt: SttRecognitionService, input: Option<String>, mut cancelled: watch::Receiver<bool>) {
    let result = async {
        let inputs = audioCall(&runtime, &recording.node, "listInputs", serde_json::json!({})).await?;
        let inputs = inputs.as_array().ok_or("Invalid microphone capabilities")?;
        let selected = match &input {
            Some(id) => inputs.iter().find(|item| item["inputId"].as_str() == Some(id)).ok_or("设备未提供所选麦克风")?,
            None => inputs.first().ok_or("当前设备未接入麦克风")?,
        };
        let inputId = selected["inputId"].as_str().ok_or("Invalid microphone input ID")?.to_string();
        if *cancelled.borrow() { return Err("语音输入已取消".to_string()); }
        let ack = audioCall(&runtime, &recording.node, "startInput", serde_json::json!({
            "streamId":id,"inputId":inputId,"format":operit_edge_contract::audio::AudioInputFormat::default(),"maxDurationMs":MAX_RECORD_MS
        })).await?;
        if ack["streamId"].as_str() != Some(&id) { return Err("Audio start acknowledgement mismatch".into()); }
        let finish = {
            let mut state = recording.state.lock().unwrap();
            state.phase = "recording";
            state.finishRequested
        };
        if finish { audioCall(&runtime, &recording.node, "finishInput", serde_json::json!({"streamId":id})).await?; }
        let mut pcm = Vec::new();
        loop {
            if *cancelled.borrow() { return Err("语音输入已取消".to_string()); }
            if recording.state.lock().unwrap().polled.elapsed() > LEASE { return Err("设备已断开，录音已取消".into()); }
            let block = tokio::select! {
                _ = cancelled.changed() => return Err("语音输入已取消".into()),
                block = recording.registry.read(&id) => block?,
            };
            if let Some(error) = block.error { return Err(error); }
            if block.done { break; }
            if block.pending { continue; }
            let bytes = STANDARD.decode(block.dataBase64).map_err(|e| e.to_string())?;
            if pcm.len() + bytes.len() > MAX_PCM_BYTES { return Err("录音超过 60 秒限制".into()); }
            pcm.extend_from_slice(&bytes);
        }
        recording.registry.close(&id);
        if *cancelled.borrow() { return Err("语音输入已取消".into()); }
        if pcm.is_empty() { return Err("没有采集到音频".into()); }
        recording.state.lock().unwrap().phase = "transcribing";
        let wav = pcmWave(pcm)?;
        #[cfg(not(target_arch = "wasm32"))]
        let result = tokio::task::spawn_blocking(move || stt.transcribeCurrent(wav, "edge-speech.wav".into(), "audio/wav".into(), None))
            .await.map_err(|e| e.to_string())??;
        #[cfg(target_arch = "wasm32")]
        let result = stt.transcribeCurrent(wav, "edge-speech.wav".into(), "audio/wav".into(), None)?;
        let text = result.text.trim().to_string();
        if text.is_empty() { return Err("没有识别到文字".into()); }
        if text.len() > 511 { return Err("识别文字超过设备输入框容量，请缩短录音".into()); }
        Ok(text)
    }.await;
    recording.registry.close(&id);
    let _ = audioCall(&runtime, &recording.node, "stopInput", serde_json::json!({"streamId": id})).await;
    let mut state = recording.state.lock().unwrap();
    state.phase = "done";
    match result { Ok(text) => state.text = Some(text), Err(error) => state.error = Some(error.chars().take(240).collect()) }
}
fn pcmWave(pcm: Vec<u8>) -> Result<Vec<u8>, String> {
    if pcm.len() > MAX_PCM_BYTES || pcm.len() % 2 != 0 { return Err("Invalid speech PCM length".into()); }
    let length = pcm.len() as u32;
    let mut wav = Vec::with_capacity(pcm.len() + 44);
    wav.extend_from_slice(b"RIFF"); wav.extend_from_slice(&(length + 36).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt "); wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&16000u32.to_le_bytes()); wav.extend_from_slice(&32000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes()); wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data"); wav.extend_from_slice(&length.to_le_bytes()); wav.extend_from_slice(&pcm);
    Ok(wav)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn speech_sessions_are_scoped_to_device_and_chat_and_errors_fit_edge_frames() {
        let registry = Arc::new(EdgeAudioRegistry::new());
        let id = registry.register("edge-a".into(), operit_edge_contract::audio::AudioInputFormat::default(), 1000).unwrap();
        let (cancel, _) = watch::channel(false);
        let speech = EdgeSpeechInput::default();
        speech.recordings.lock().unwrap().insert(id.clone(), Arc::new(Recording {
            chat:"chat-a".into(), node:"edge-a".into(), registry, cancel,
            state:Mutex::new(State {phase:"recording",text:None,error:None,finishRequested:false,polled:Instant::now()})
        }));
        assert_eq!(speech.poll("chat-a","edge-a",&id).unwrap()["phase"],"recording");
        assert!(speech.poll("chat-b","edge-a",&id).is_err());
        assert!(speech.poll("chat-a","edge-b",&id).is_err());
        let failure = reply(Err("错".repeat(10000)));
        let wire = operit_link::encodeLink(&operit_link::toCoreValue(failure).unwrap()).unwrap();
        assert!(wire.len() < 1024);
    }
    #[test]
    fn wav_preserves_pcm_and_declares_local_stt_format() {
        let wav = pcmWave(vec![0, 0, 255, 127, 0, 128]).unwrap();
        assert_eq!(&wav[..4], b"RIFF"); assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16000);
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16);
        assert_eq!(&wav[44..], &[0, 0, 255, 127, 0, 128]);
        assert!(pcmWave(vec![0]).is_err()); assert!(pcmWave(vec![0; MAX_PCM_BYTES + 2]).is_err());
    }
}
