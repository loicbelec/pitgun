// Offline diagnostic adapter. Executes only the operator-pinned Racing WASM;
// never authorizes/submits a run or calls a controller/provider. Production's
// services/pitgun-verifier/wasm-service-replay.mjs remains unchanged.
import fs from 'node:fs';
import os from 'node:os';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
import {performance,PerformanceObserver} from 'node:perf_hooks';
const [payloadFile,expectedFile,wasmFile,bridgeFile,wasmDigest,bridgeDigest,repetitions='1',transport='presentation']=process.argv.slice(2);
assert.ok(bridgeDigest,'Pass payload expected wasm bridge wasmDigest bridgeDigest [repetitions]');
const count=Number(repetitions);assert.ok(Number.isInteger(count)&&count>=1&&count<=3);
assert.ok(['presentation','replay'].includes(transport));
const hash=b=>'sha256:'+createHash('sha256').update(b).digest('hex');
const stable=v=>JSON.stringify(sort(v));
function sort(v){if(Array.isArray(v))return v.map(sort);if(v&&typeof v==='object')return Object.fromEntries(Object.keys(v).sort().map(k=>[k,sort(v[k])]));return v;}
const timings=[];
function timed(name,fn,metadata={}){
 const cpu=process.cpuUsage(),start=performance.now(),value=fn();
 timings.push({name,wallMs:performance.now()-start,cpuUs:process.cpuUsage(cpu),...metadata});return value;
}
const began=performance.now(),initialUsage=process.resourceUsage();
const payloadBytes=timed('input.read',()=>fs.readFileSync(payloadFile));assert.ok(payloadBytes.length<=32*1024*1024);
const input=timed('input.parse',()=>JSON.parse(payloadBytes.toString('utf8')));
const expected=JSON.parse(fs.readFileSync(expectedFile,'utf8'));
const wasm=timed('runtime.read',()=>fs.readFileSync(wasmFile)),bridge=fs.readFileSync(bridgeFile);
timed('runtime.hash',()=>{assert.equal(hash(wasm),wasmDigest);assert.equal(hash(bridge),bridgeDigest);});
const importStart=performance.now(),api=await import(pathToFileURL(bridgeFile).href);
timings.push({name:'bridge.import',wallMs:performance.now()-importStart});
const initStart=performance.now(),exports=await api.default({module_or_path:wasm});
timings.push({name:'wasm.initialize',wallMs:performance.now()-initStart});
const initialization=timings.splice(0),gc=[];
const collectGc=entries=>{for(const e of entries)gc.push({atMs:e.startTime,durationMs:e.duration,kind:e.detail?.kind});};
const observer=new PerformanceObserver(list=>collectGc(list.getEntries()));
observer.observe({entryTypes:['gc']});
const parse=raw=>{const r=JSON.parse(raw);if(r.error)throw Error(r.error);return r;};
const request=JSON.parse(input.requestJson),config=JSON.parse(input.configJson);
const pull=transport==='replay'?api.pull_experimental_controlled_racing_replay_json:api.pull_experimental_controlled_racing_session_json;
const complete=transport==='replay'?api.complete_experimental_controlled_racing_replay_json:api.complete_experimental_controlled_racing_session_json;
assert.equal(typeof pull,'function');assert.equal(typeof complete,'function');
const runs=[];
for(let run=0;run<count;run++){
 const start=performance.now(),cpu=process.cpuUsage();
 const memory=[],calls=[];let memorySamplingMs=0,wireBytes=0,index=0,output,handle;
 const sample=label=>{const t=performance.now();memory.push({label,atMs:t-start,...process.memoryUsage(),wasmLinearBytes:exports.memory?.buffer.byteLength??null});memorySamplingMs+=performance.now()-t;};
 const call=(name,fn,extra={})=>{
  const raw=timed(name+'.abi',fn,extra);
  const bytes=Buffer.byteLength(raw);wireBytes+=bytes;
  const parsed=timed(name+'.parse',()=>parse(raw),extra);
  calls.push({name,bytes,...extra});return parsed;
 };
 sample('before-start');
 try{
  const started=call('start',()=>api.start_experimental_controlled_racing_session_json(input.requestJson,input.catalogBundleJson,input.timelineJson,input.configJson));
  handle=started.handle;sample('after-start');
  for(let pulls=0;pulls<100000;pulls++){
   const next=call('pull',()=>pull(handle),{pull:pulls});
   sample('pull-'+pulls+'-'+next.kind);
   if(next.kind==='decision'){
    const record=input.history[index++];assert.ok(record,'Missing recorded decision');
    assert.equal(record.observation_digest,next.observation_digest,'Regenerated observation differs');
    const proposal=timed('proposal.encode',()=>JSON.stringify(record.applied.proposal));
    const accepted=call('propose',()=>api.propose_experimental_controlled_racing_session_json(handle,proposal));
    timed('receipt.compare',()=>assert.equal(stable(accepted.receipt),stable(record.applied),'Regenerated receipt differs'));
   }else if(next.kind==='batch'&&next.complete){
    assert.equal(index,input.history.length,'Extra recorded decisions');
    const completed=call('complete',()=>complete(handle));handle=undefined;
    assert.ok(completed.native_evidence,'Missing native evidence');output=completed.native_evidence;break;
   }else assert.equal(next.kind,'batch','Unexpected control response');
  }
  assert.ok(output,'Replay pull bound exceeded');sample('after-completion');
  const evidenceDigest=timed('evidence.compare',()=>{assert.equal(stable(output),stable(expected),'Output or telemetry summary differs');return hash(stable(output));});
  const encoded=timed('reply.encode',()=>JSON.stringify({ok:true,evidence:output}));assert.ok(Buffer.byteLength(encoded)<=65536);
  await new Promise(setImmediate);sample('after-observer-drain');
  const wallMs=performance.now()-start,cpuUs=process.cpuUsage(cpu),phases={};
  for(const t of timings){const v=phases[t.name]??={calls:0,wallMs:0,userCpuUs:0,systemCpuUs:0};v.calls++;v.wallMs+=t.wallMs;v.userCpuUs+=t.cpuUs?.user??0;v.systemCpuUs+=t.cpuUs?.system??0;}
  runs.push({run,temperature:run===0?'first-session-after-module-init':'same-module-session-reuse',wallMs,cpuUs,phases,wireBytes,replyBytes:Buffer.byteLength(encoded),evidenceDigest,historyCount:index,memorySamplingMs,memorySamples:memory,resourceUsage:process.resourceUsage(),gcWindow:{fromMs:start,toMs:start+wallMs},calls,perCallTimings:timings.splice(0)});
 }finally{if(handle!==undefined)api.release_experimental_controlled_racing_session_json(handle);}
}
// Observer delivery can lag across setImmediate/next session. Attribute GC by
// its actual timestamp, never by the callback's delivery turn.
await new Promise(resolve=>setTimeout(resolve,20));collectGc(observer.takeRecords());observer.disconnect();
for(const run of runs){const events=gc.filter(e=>e.atMs>=run.gcWindow.fromMs&&e.atMs<run.gcWindow.toMs);run.gc={count:events.length,observerDurationMs:events.reduce((n,e)=>n+e.durationMs,0)};}
const report={schemaVersion:'pitgun.offline-service-replay-profile/v1',passed:true,scope:'Offline exact pinned-WASM replay parity and phase timing, not a new verifier verdict or cross-architecture speed claim',environment:{node:process.version,v8:process.versions.v8,platform:process.platform,arch:process.arch,cpuModel:os.cpus()[0]?.model,cpuCount:os.cpus().length,totalMemoryBytes:os.totalmem()},identity:{wasmDigest,bridgeDigest,payloadDigest:hash(payloadBytes),inputDigest:hash(input.requestJson),expectedDigest:hash(stable(expected)),circuit:request.input.track_id,laps:request.input.laps,cars:request.input.competitors.length,hz:request.hz,physics:config.physics},initialization,runs,totalWallMs:performance.now()-began,initialResourceUsage:initialUsage,resourceUsage:process.resourceUsage(),limitations:['ABI time includes Rust advance, serialization and WASM/JS string copying; it is not pure solver time.','GC duration overlaps measured phases; do not add it to total.','Memory snapshots are not a guaranteed instantaneous peak; maxRSS is process-lifetime high-water mark.','Same-module reuse does not represent independent cold worker processes.','No API, scheduler wait, browser renderer or network time is measured.']};
report.transport=transport;
process.stdout.write(JSON.stringify(report,null,2)+'\n');
