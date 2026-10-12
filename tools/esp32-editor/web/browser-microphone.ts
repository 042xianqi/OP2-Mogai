import type {DeviceState} from './device-state.js';
import {SpeechPcm} from './speech-pcm.ts';

/** Browser hardware is an Edge source. STT runs on the admitted Core. */
export class BrowserMicrophone {
  private stream?: MediaStream;
  private context?: AudioContext;
  private source?: MediaStreamAudioSourceNode;
  private processor?: AudioWorkletNode;
  private pcm?: SpeechPcm;
  private captureId?: string;
  private sessionId?: string;
  private pending: Uint8Array[] = [];
  private sending?: Promise<void>;
  private starting = false;
  private stopped = false;
  private generation = 0;

  update(state: DeviceState): void {
    if (this.starting && (!state.running || !state.connected)) { this.release(); return; }
    if (this.sessionId && (state.sessionId !== this.sessionId || !state.connected ||
        (this.captureId && state.chat?.speech?.phase !== 1))) this.release();
  }
  async toggle(state: DeviceState, invoke: (inputId: string) => Promise<unknown>): Promise<void> {
    if (this.starting) throw new Error('正在准备麦克风');
    if (state.chat?.speech?.phase === 1) {
      this.stopped = true;
      this.processor?.disconnect(); this.source?.disconnect();
      const tail = this.pcm?.flush(); if (tail?.length) this.pending.push(tail);
      try { await this.drain(); } catch (error) { await this.fail(String(error)); throw error; }
      try { await invoke('browser-mic'); } finally { this.release(); }
      return;
    }
    this.starting = true;
    const generation = ++this.generation;
    try {
      if (!navigator.mediaDevices?.getUserMedia) throw new Error('浏览器无法访问麦克风，请使用 localhost 或 HTTPS');
      this.stream = await navigator.mediaDevices.getUserMedia({audio: {channelCount: 1, echoCancellation: true}, video: false});
      if (generation !== this.generation) throw new Error("设备连接已变化");
      this.context = new AudioContext();
      await this.context.resume();
      this.pcm = new SpeechPcm(this.context.sampleRate);
      const source = `class Capture extends AudioWorkletProcessor {
        process(inputs) {
          const channels = inputs[0];
          if (channels?.length) {
            const mono = new Float32Array(channels[0].length);
            for (const channel of channels) for (let i=0;i<mono.length;i++) mono[i]+=channel[i]/channels.length;
            this.port.postMessage(mono, [mono.buffer]);
          }
          return true;
        }
      } registerProcessor('edge-capture', Capture);`;
      const url = URL.createObjectURL(new Blob([source], {type: 'text/javascript'}));
      try { await this.context.audioWorklet.addModule(url); } finally { URL.revokeObjectURL(url); }
      await invoke('browser-mic');
      const deadline = Date.now() + 15000;
      let device: DeviceState | undefined;
      while (Date.now() < deadline) {
        const response = await fetch('/api/simulator/state');
        const result = await response.json() as {device?: DeviceState};
        device = result.device;
        if (!response.ok || device?.chat?.speech?.error) throw new Error(device?.chat?.speech?.error ?? '设备连接失败');
        if (device?.audio?.captureId && device.chat?.speech?.phase === 1) break;
        if (generation !== this.generation) throw new Error('设备连接已变化');
        await new Promise(resolve => setTimeout(resolve, 100));
      }
      if (!device?.audio?.captureId || device.chat?.speech?.phase !== 1) throw new Error('Core 未启动浏览器麦克风，请检查 STT 配置和设备连接');
      if (generation !== this.generation) throw new Error('设备连接已变化');
      this.captureId = device.audio.captureId; this.sessionId = device.sessionId;
      this.stopped = false; this.pending = [];
      this.source = this.context.createMediaStreamSource(this.stream);
      this.processor = new AudioWorkletNode(this.context, 'edge-capture');
      this.processor.port.onmessage = event => {
        if (this.stopped) return;
        const blocks = this.pcm!.push(event.data as Float32Array);
        if (this.pending.length + blocks.length > 8) { void this.fail('麦克风上传过载'); return; }
        this.pending.push(...blocks);
        void this.drain().catch(error => this.fail(String(error)));
      };
      this.source.connect(this.processor); this.processor.connect(this.context.destination);
      for (const track of this.stream.getTracks()) track.onended = () => { if (!this.stopped) void this.fail('麦克风已断开'); };
    } catch (error) { await this.fail(String(error)); throw error; }
    finally { this.starting = false; }
  }
  private async drain(): Promise<void> {
    if (this.sending) return this.sending;
    this.sending = (async () => {
      while (this.pending.length && this.captureId) {
        const pcm = this.pending.shift()!;
        const response = await fetch('/api/simulator/audio/pcm', {method: 'POST',
          headers: {'Content-Type': 'application/octet-stream', 'X-Operit-Capture-Id': this.captureId},
          body: pcm as Uint8Array<ArrayBuffer>});
        if (!response.ok) { const detail = await response.json() as {error?: string}; throw new Error(detail.error ?? '音频上传失败'); }
      }
    })();
    try { await this.sending; } finally { this.sending = undefined; }
  }
  private async fail(error: string): Promise<void> {
    const captureId = this.captureId;
    this.release();
    if (captureId) await fetch('/api/simulator/audio/error', {method: 'POST',
      headers: {'Content-Type': 'application/json'}, body: JSON.stringify({captureId, error: error.slice(0, 120)})}).catch(() => {});
  }
  release(): void {
    this.generation++; this.stopped = true;
    this.processor?.disconnect(); this.source?.disconnect();
    this.stream?.getTracks().forEach(track => track.stop());
    void this.context?.close();
    this.stream = undefined; this.context = undefined; this.source = undefined; this.processor = undefined;
    this.pcm = undefined; this.captureId = undefined; this.sessionId = undefined; this.pending = [];
  }
}
