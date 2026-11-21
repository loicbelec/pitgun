// Offline reference-controller fixture preparation for a NEW candidate binary.
// Never retag an old signature/history with a different WASM identity.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
const [payloadFile,wasmFile,bridgeFile,directory,legacyExpected]=process.argv.slice(2);
assert.ok(directory,'Pass payload wasm bridge output-directory [legacy-expected]');
const hash=b=>'sha256:'+createHash('sha256').update(b).digest('hex');
const stable=v=>JSON.stringify(sort(v));
function sort(v){if(Array.isArray(v))return v.map(sort);if(v&&typeof v==='object')return Object.fromEntries(Object.keys(v).sort().map(k=>[k,sort(v[k])]));return v;}
const sourceBytes=fs.readFileSync(payloadFile),input=JSON.parse(sourceBytes),config=JSON.parse(input.configJson);
assert.equal(config.signed_service,undefined,'Candidate fixture cannot reuse a signed service sidecar');
const bytes=fs.readFileSync(wasmFile),bridge=fs.readFileSync(bridgeFile),runtimeDigest=hash(bytes);
const oldRuntime=config.runtime_digest;
assert.notEqual(runtimeDigest,oldRuntime,'Candidate must have a distinct artifact identity');
config.runtime_digest=runtimeDigest;config.execution_id='offline-compact-'+runtimeDigest.slice(7,23);
const candidate={...input,configJson:JSON.stringify(config),history:[]};
const api=await import(pathToFileURL(path.resolve(bridgeFile)).href);await api.default({module_or_path:bytes});
const parse=raw=>{const r=JSON.parse(raw);assert.equal(r.error,undefined,JSON.stringify(r));return r;};
const started=parse(api.start_experimental_controlled_racing_session_json(candidate.requestJson,candidate.catalogBundleJson,candidate.timelineJson,candidate.configJson));
let active=true,expected;
try{
 for(let pulls=0;pulls<100000;pulls++){
  const next=parse(api.pull_experimental_controlled_racing_replay_json(started.handle));
  if(next.kind==='decision'){
   const index=candidate.history.length,o=next.observation;
   const original=input.history?.[index];
   if(input.history)assert.ok(original,'Candidate opened an extra decision window');
   const action=original?.applied.proposal.action??input.referenceActions?.[index]??{kind:'keep_plan'};
   const source=input.referenceFallback?{kind:'fallback',reason:'timeout'}:{kind:'reference_controller'};
   const proposal={version:o.version,run_digest:o.run_digest,decision_sequence:o.decision_sequence,
    completed_laps:o.completed_laps,observation_digest:next.observation_digest,idempotency_key:'offline-reference-'+index,source,action};
   if(o.window)proposal.decision_at_us=input.referenceFallback?o.window.deadline_at_us:Math.min(o.window.opened_at_us+100000,o.window.deadline_at_us);
   const applied=parse(api.propose_experimental_controlled_racing_session_json(started.handle,JSON.stringify(proposal))).receipt;
   candidate.history.push({observation_digest:next.observation_digest,applied});
  }else if(next.kind==='batch'&&next.complete){
   const completed=parse(api.complete_experimental_controlled_racing_replay_json(started.handle));active=false;
   expected=completed.native_evidence;break;
  }else assert.equal(next.kind,'batch');
 }
 assert.ok(expected,'No terminal evidence');
 if(input.history)assert.equal(candidate.history.length,input.history.length,'Candidate omitted a decision window');
 if(legacyExpected)assert.equal(stable(expected),stable(JSON.parse(fs.readFileSync(legacyExpected,'utf8'))),'Candidate changed legacy output or telemetry summary');
 delete candidate.referenceActions;delete candidate.referenceFallback;
 fs.mkdirSync(directory,{recursive:true,mode:0o700});
 fs.writeFileSync(path.join(directory,'payload.json'),JSON.stringify(candidate),{mode:0o600});
 fs.writeFileSync(path.join(directory,'expected.json'),JSON.stringify(expected),{mode:0o600});
 const manifest={scope:'Offline reference-controller fixture; new runtime/context/receipts, not an Authority admission or a Gemini execution',sourcePayloadDigest:hash(sourceBytes),sourceRuntime:oldRuntime,
  runtimeDigest,bridgeDigest:hash(bridge),payloadDigest:hash(JSON.stringify(candidate)),evidenceDigest:hash(stable(expected)),historyCount:candidate.history.length,physicalRequestUnchanged:candidate.requestJson===input.requestJson,legacyCompactParity:!!legacyExpected};
 fs.writeFileSync(path.join(directory,'manifest.json'),JSON.stringify(manifest,null,2)+'\n',{mode:0o600});
 console.log(JSON.stringify(manifest));
}finally{if(active)api.release_experimental_controlled_racing_session_json(started.handle);}
