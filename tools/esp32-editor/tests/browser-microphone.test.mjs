import test from 'node:test';
import assert from 'node:assert/strict';
import {BrowserMicrophone} from '../web/browser-microphone.ts';

function browser(t, deny=false) {
  const saved = new Map(['navigator','AudioContext','AudioWorkletNode','fetch'].map(key=>[key,Object.getOwnPropertyDescriptor(globalThis,key)]));
  const requests=[], contexts=[], processors=[], track={stopped:false,stop(){this.stopped=true;}};
  Object.defineProperty(globalThis,'navigator',{configurable:true,value:{mediaDevices:{getUserMedia:async()=>{
    if(deny)throw Error('permission denied'); return {getTracks:()=>[track]};
  }}}});
  globalThis.AudioContext=class {
    sampleRate=48000; destination={}; audioWorklet={addModule:async()=>{}};
    constructor(){contexts.push(this);} async resume(){} async close(){this.closed=true;}
    createMediaStreamSource(){return {connect(){},disconnect(){}};}
  };
  globalThis.AudioWorkletNode=class {port={};constructor(){processors.push(this);}connect(){}disconnect(){}};
  globalThis.fetch=async(url,options)=>{
    requests.push({url,options});
    return {ok:true,json:async()=>({device:{sessionId:'one',audio:{captureId:'01234567-1234-1234-1234-012345678901'},chat:{speech:{phase:1}}}})};
  };
  t.after(()=>{for(const[key,value]of saved){if(value)Object.defineProperty(globalThis,key,value);else delete globalThis[key];}});
  return {requests,contexts,processors,track};
}
test('browser permission denial never starts Edge capture', async t=>{
  const fixture=browser(t,true), mic=new BrowserMicrophone(); let calls=0;
  await assert.rejects(mic.toggle({},async()=>{calls++;}),/permission denied/);
  assert.equal(calls,0); assert.equal(fixture.requests.length,0);
});
test('real browser samples go to Edge PCM and finish flushes before remote stop',async t=>{
  const fixture=browser(t),mic=new BrowserMicrophone(),calls=[];
  await mic.toggle({},async id=>{calls.push(id);});
  for(let i=0;i<4;i++){
    fixture.processors[0].port.onmessage({data:Float32Array.from({length:512},()=>0.5)});
    await new Promise(done=>setImmediate(done));
  }
  await mic.toggle({chat:{speech:{phase:1}}},async()=>{
    assert(fixture.requests.some(r=>r.url==='/api/simulator/audio/pcm'));
    assert.equal(fixture.track.stopped,false,'flush before closing device');calls.push('finish');
  });
  const uploads=fixture.requests.filter(r=>r.url==='/api/simulator/audio/pcm');
  assert(uploads.every(r=>r.options.body.length>0 && r.options.body.length<=640 && r.options.body.length%2===0));
  assert(uploads.every(r=>r.options.headers['X-Operit-Capture-Id']==='01234567-1234-1234-1234-012345678901'));
  assert.deepEqual(calls,['browser-mic','finish']);assert(fixture.track.stopped);assert(fixture.contexts[0].closed);
});
test('connection change releases microphone and later frames cannot upload',async t=>{
  const fixture=browser(t),mic=new BrowserMicrophone();await mic.toggle({},async()=>{});
  mic.update({sessionId:'two',running:true,connected:true,chat:{speech:{phase:1}}});
  const before=fixture.requests.length;
  fixture.processors[0].port.onmessage({data:new Float32Array(512)});
  await new Promise(done=>setImmediate(done));
  assert.equal(fixture.requests.length,before);assert(fixture.track.stopped);
});
