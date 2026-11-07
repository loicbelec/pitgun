// Same local JSON API as the native facade diagnostic; no browser control.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';
const [inputPath,modulePath,binaryPath,reportPath]=process.argv.slice(2);
assert.ok(reportPath,'INPUT MODULE BINARY REPORT');
const input=JSON.parse(await readFile(inputPath,'utf8'));
const bytes=await readFile(binaryPath);
const api=await import(pathToFileURL(resolve(modulePath)).href);
const initialization=performance.now(); const instance=api.initSync({module:bytes});
const initialize_ms=performance.now()-initialization;
const parse=raw=>{const r=JSON.parse(raw);assert.equal(r.error,undefined);return r;};
const started=performance.now();
let begin=performance.now();
const raw=api.start_local_racing_session_with_catalog_and_v3_power_unit_thermal_profile_json(
  input.request_json,input.catalog_bundle_json,input.profile_json);
const setup_ms=performance.now()-begin;
begin=performance.now();const start=parse(raw);let host_parse_ms=performance.now()-begin;
const pull_api_ms=[];let stream_bytes=0;
const computeBegin=performance.now();
for(;;){
  begin=performance.now();const raw=api.pull_local_racing_session_json(start.handle);
  pull_api_ms.push(performance.now()-begin);stream_bytes+=Buffer.byteLength(raw);
  begin=performance.now();const batch=parse(raw);host_parse_ms+=performance.now()-begin;
  if(batch.complete)break;
}
const compute_ms=performance.now()-computeBegin;
begin=performance.now();const final=api.complete_local_racing_session_json(start.handle);
const finish_ms=performance.now()-begin;
begin=performance.now();const result=parse(final).result;host_parse_ms+=performance.now()-begin;
const elapsed_ms=performance.now()-started;
const report={runtime:'wasm-node',node:process.version,mode:'json',initialize_ms,setup_ms,
  compute_ms,finish_ms,host_parse_ms,elapsed_ms,pull_api_ms,stream_bytes,
  completion_bytes:Buffer.byteLength(final),
  frames:result.player_batches.reduce((n,b)=>n+b.frames.length,0),
  standings:result.standings,player_lap_times_ms:result.player_lap_times_ms,
  wasm_memory_bytes:instance.memory.buffer.byteLength,
  notes:'Fresh Node process; RSS includes host parsing/validation. Not Chrome/Safari performance or pure Solver time.'};
await writeFile(reportPath,JSON.stringify(report,null,2)+'\n');
