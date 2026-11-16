// Offline protocol/failure checks against a candidate's short multi-window fixture.
// No Authority, provider or network. Each case uses a fresh bounded Node child.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const [payloadFile,expectedFile,wasmFile,bridgeFile,adapterFile,stockAdapter]=process.argv.slice(2);
assert.ok(stockAdapter,'Pass payload expected wasm bridge compact-adapter stock-adapter');
const hash=file=>'sha256:'+createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const original=JSON.parse(fs.readFileSync(payloadFile,'utf8')),expected=JSON.parse(fs.readFileSync(expectedFile,'utf8'));
assert.ok(original.history.length>=2,'Use a short fixture with at least two windows');
const wasmDigest=hash(wasmFile),bridgeDigest=hash(bridgeFile),results=[];
const invoke=(input,adapter=adapterFile,pin=wasmDigest)=>{
 const paths=[wasmFile,bridgeFile,adapter].map(file=>path.resolve(file));
 const r=spawnSync(process.execPath,['--experimental-permission','--max-old-space-size=512',...paths.map(file=>'--allow-fs-read='+file),...paths.slice(2),paths[0],paths[1],pin,bridgeDigest],{input:JSON.stringify(input),encoding:'utf8',timeout:30000,maxBuffer:65536,env:{}});
 assert.equal(r.error,undefined,String(r.error));assert.equal(r.status,0,r.stderr);return JSON.parse(r.stdout);
};
for(const adapter of [stockAdapter,adapterFile]){
 const response=invoke(original,adapter);assert.equal(response.ok,true,JSON.stringify(response));assert.deepEqual(response.evidence,expected);
 results.push({case:adapter===stockAdapter?'presentation-parity':'compact-parity',passed:true});
}
const cases=[
 ['missing-history',p=>{p.history=[];}],
 ['extra-history',p=>{p.history.push(structuredClone(p.history.at(-1)));}],
 ['reordered-history',p=>{p.history.reverse();}],
 ['wrong-observation',p=>{p.history[0].observation_digest='sha256:'+'0'.repeat(64);}],
 ['wrong-receipt',p=>{p.history[0].applied.pit_after_lap=99;}],
 ['wrong-run',p=>{p.history[0].applied.proposal.run_digest='sha256:'+'0'.repeat(64);}],
 ['late-decision',p=>{p.history[0].applied.proposal.decision_at_us=Number.MAX_SAFE_INTEGER;}],
 ['undelegated-tire',p=>{p.history[0].applied.proposal.action={kind:'pit_next_lap',tire_id:'not-delegated'};}],
];
for(const [name,mutate]of cases){const input=structuredClone(original);mutate(input);const r=invoke(input);assert.equal(r.ok,false,name);assert.equal(r.code,'replay_mismatch',JSON.stringify(r));results.push({case:name,passed:true});}
// The pin is checked before stdin is consumed; keep this payload tiny so early
// rejection does not make the test driver write a large body to a closed pipe.
const invalid=invoke({},adapterFile,'sha256:'+'0'.repeat(64));assert.equal(invalid.ok,false);assert.equal(invalid.code,'runtime_unavailable');assert.match(invalid.error,/Pinned runtime/);results.push({case:'wrong-runtime-pin',passed:true});
console.log(JSON.stringify({passed:true,scope:'Offline experimental adapter parity and hostile-history cases; not a hosted signature/verdict test',wasmDigest,bridgeDigest,results},null,2));
