// Offline replay only; input contains a prior QA authorization, never a signing key.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';
const [inputPath,modulePath,binaryPath,reportPath,dumpPath]=process.argv.slice(2);
assert.ok(reportPath,'INPUT MODULE BINARY REPORT [PRIVATE_RAW_OUTPUT]');
const input=JSON.parse(await readFile(inputPath,'utf8'));
const request=JSON.parse(input.request_json);
assert.equal(request.signed_authorization.authorization.initial_contract.model.version,'0.14.0');
const bytes=await readFile(binaryPath);
const api=await import(pathToFileURL(resolve(modulePath)).href);
const instance=api.initSync({module:bytes});
const snap=phase=>({phase,...process.memoryUsage(),wasm_bytes:instance.memory.buffer.byteLength});
const memory_snapshots=[snap('before_start')];
const parse=raw=>{const r=JSON.parse(raw);assert.equal(r.error,undefined);return r;};
let begin=performance.now();
const raw=api.start_authorized_dynamic_racing_session_with_catalog_json(input.request_json,input.catalog_bundle_json);
const setup_ms=performance.now()-begin;
begin=performance.now();const start=parse(raw);let host_parse_ms=performance.now()-begin;
memory_snapshots.push(snap('after_setup'));
const pull_api_ms=[];let stream_bytes=0;
for(;;){
  begin=performance.now();const raw=api.pull_authorized_dynamic_racing_session_json(start.handle);
  pull_api_ms.push(performance.now()-begin);stream_bytes+=Buffer.byteLength(raw);
  begin=performance.now();const batch=parse(raw);host_parse_ms+=performance.now()-begin;
  if(pull_api_ms.length===request.input.laps)memory_snapshots.push(snap('after_last_lap_before_finalization'));
  if(batch.complete){memory_snapshots.push(snap('after_terminal_pull'));break;}
}
begin=performance.now();const final=api.complete_authorized_dynamic_racing_session_json(start.handle);
const completion_ms=performance.now()-begin;
begin=performance.now();const completed=parse(final);host_parse_ms+=performance.now()-begin;
memory_snapshots.push(snap('after_completion'));
const result=completed.result.runtime_output;
const report={runtime:'wasm-node',node:process.version,model:'0.14.0',setup_ms,pull_api_ms,
  completion_ms,host_parse_ms,stream_bytes,completion_bytes:Buffer.byteLength(final),memory_snapshots,
  frames:result.player_batches.reduce((n,b)=>n+b.frames.length,0),
  run_id:completed.result.evidence.receipt.receipt.run_id,
  standings:result.standings,player_lap_times_ms:result.player_lap_times_ms};
await writeFile(reportPath,JSON.stringify(report,null,2)+'\n');
// Dumps are separate untimed equivalence runs; never use their process RSS.
if(dumpPath)await writeFile(dumpPath,final,{mode:0o600});
