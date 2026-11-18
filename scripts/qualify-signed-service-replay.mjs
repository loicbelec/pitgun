// Disposable loopback-only Authority -> actual WASM -> HTTP Verifier recipe.
// No browser, provider, deployed services, persistent database or real signing key.
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import {createHash,randomBytes} from 'node:crypto';
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {fileURLToPath,pathToFileURL} from 'node:url';
import assert from 'node:assert/strict';
const run=promisify(execFile),self=fileURLToPath(import.meta.url);
const root=path.resolve(path.dirname(self),'..');
const hash=b=>'sha256:'+createHash('sha256').update(b).digest('hex');
const parse=raw=>{const v=JSON.parse(raw);assert.equal(v.error,undefined,JSON.stringify(v));return v;};

if(process.argv[2]==='--execute'){
 const [payloadFile,wasm,bridge,mode,destination,completionTransport='full']=process.argv.slice(3);
 assert.ok(['full','metadata'].includes(completionTransport));
 const input=JSON.parse(await fs.readFile(payloadFile,'utf8'));
 const api=await import(pathToFileURL(bridge).href);
 await api.default({module_or_path:await fs.readFile(wasm)});
 const started=parse(api.start_experimental_controlled_racing_session_json(input.requestJson,input.catalogBundleJson,input.timelineJson,input.configJson));
 let active=true;
 try{
  for(let pulls=0;pulls<100000;pulls++){
   const next=parse(api.pull_experimental_controlled_racing_session_json(started.handle));
   if(next.kind==='decision'){
    const o=next.observation;
    const proposal={version:o.version,run_digest:o.run_digest,decision_sequence:o.decision_sequence,
     completed_laps:o.completed_laps,observation_digest:next.observation_digest,
     idempotency_key:'qualification-'+o.decision_sequence,
     source:mode==='timeout'?{kind:'fallback',reason:'timeout'}:{kind:'reference_controller'},
     action:mode==='pit'&&o.decision_sequence===0?{kind:'pit_next_lap',tire_id:'hard'}:{kind:'keep_plan'}};
    if(o.window)proposal.decision_at_us=mode==='timeout'?o.window.deadline_at_us:Math.min(o.window.opened_at_us+100000,o.window.deadline_at_us);
    parse(api.propose_experimental_controlled_racing_session_json(started.handle,JSON.stringify(proposal)));
   }else if(next.kind==='batch'&&next.complete){
    const complete=completionTransport==='metadata'?api.complete_experimental_controlled_racing_metadata_json:api.complete_experimental_controlled_racing_session_json;
    assert.equal(typeof complete,'function','Selected completion transport must be explicit');
    const completion=parse(complete(started.handle));
    assert.equal(completion.schema_version,completionTransport==='metadata'?'pitgun.racing-control-metadata/v1':'pitgun.racing-control-completion/v1');
    assert.equal(Object.hasOwn(completion,'result'),completionTransport==='full');
    assert.match(completion.canonical_result_digest,/^sha256:[0-9a-f]{64}$/);
    assert.ok(Array.isArray(completion.journal));
    active=false;assert.ok(completion.service_evidence,'Signed WASM completion must carry evidence');
    // Normalize like the browser host for the separate forged-evidence probe.
    // The metadata ABI itself must not contain result (asserted above).
    const terminal=next.batch.records.at(-1)?.event;
    assert.equal(terminal?.kind,'complete');
    const normalized=completionTransport==='metadata'?{...completion,schema_version:'pitgun.racing-control-completion/v1',result:terminal.payload}:completion;
    await fs.writeFile(destination,JSON.stringify(normalized),{mode:0o600});
    process.exitCode=0;break;
   }else assert.equal(next.kind,'batch');
  }
  assert.equal(active,false,'Client pull bound exceeded');
 }finally{if(active)api.release_experimental_controlled_racing_session_json(started.handle);}
}else{
 const args=process.argv.slice(2,7).map(p=>path.resolve(p));
 const completionTransport=process.argv[7]??'full';
 assert.ok(['full','metadata'].includes(completionTransport),'Optional completion transport: full|metadata');
 assert.equal(args.length,5,'Pass CANDIDATE_WASM CANDIDATE_BRIDGE LEGACY_WASM LEGACY_BRIDGE NEW_PRIVATE_OUTPUT_DIR');
 const [wasm,bridge,oldWasm,oldBridge,directory]=args;
 await fs.mkdir(directory,{mode:0o700});
 const temporary=await fs.mkdtemp(path.join(os.tmpdir(),'pitgun-signed-compact-'));
 const catalog=path.join(root,'catalogs/racing/v1.8.1');
 const pins={candidate:{wasm:hash(await fs.readFile(wasm)),bridge:hash(await fs.readFile(bridge))},
  legacy:{wasm:hash(await fs.readFile(oldWasm)),bridge:hash(await fs.readFile(oldBridge))}};
 assert.notEqual(pins.candidate.wasm,pins.legacy.wasm,'Never replace a historical artifact');
 const children=[],checks=[];
 const serverWasm=path.join(directory,'server-engine.wasm');
 await fs.copyFile(wasm,serverWasm);
 const write=async(name,value)=>{const file=path.join(directory,name);await fs.writeFile(file,JSON.stringify(value),{mode:0o600});return file;};
 const common={PATH:process.env.PATH,NO_COLOR:'1',RUST_LOG:'info',PITGUN_SIGNING_KEY_ID:'disposable-replay-qa',
  PITGUN_SIGNING_SECRET_FILE:path.join(temporary,'signing'),PITGUN_TUNING_POLICY_PATH:path.join(root,'policies/gametuning.v1.yaml'),
  PITGUN_RACING_CATALOG_RELEASE_DIR:path.join(root,'catalogs/racing/v1.0.0'),
  PITGUN_RACING_ATTEMPT_CATALOG_RELEASE_DIR:path.join(root,'catalogs/racing/v1.8.0'),
  PITGUN_RACING_SERVICE_CATALOG_RELEASE_DIR:catalog,PITGUN_RACING_SERVICE_WASM_DIGEST:pins.candidate.wasm,
  PITGUN_SERVICE_NODE_BINARY:process.execPath,PITGUN_SERVICE_REPLAY_SCRIPT:path.join(root,'services/pitgun-verifier/wasm-service-replay-compact.mjs'),
  PITGUN_SERVICE_WASM_FILE:serverWasm,PITGUN_SERVICE_BRIDGE_FILE:bridge,PITGUN_SERVICE_BRIDGE_DIGEST:pins.candidate.bridge,
  PITGUN_VERIFIER_MAX_CONCURRENT_REPLAYS:'1'};
 async function launch(binary,bind,extra={}){
  const child=spawn(path.join(root,'target/release',binary),[],{cwd:root,env:{...common,[bind]:'127.0.0.1:0',...extra},stdio:['ignore','pipe','pipe']});
  children.push(child);let logs='',failure;
  child.on('error',e=>{failure=e;});
  const collect=b=>{logs=(logs+b.toString()).slice(-65536);};
  child.stdout.on('data',collect);child.stderr.on('data',collect);
  for(let i=0;i<200;i++){
   if(failure)throw failure;
   const match=logs.replace(/\x1b\[[0-9;]*m/g,'').match(/listening on (127\.0\.0\.1:\d+)/);
   if(match)return 'http://'+match[1];
   if(child.exitCode!==null){await fs.writeFile(path.join(directory,binary+'-failure.log'),logs,{mode:0o600});throw Error(binary+' failed before ready');}
   await new Promise(r=>setTimeout(r,50));
  }
  throw Error(binary+' startup timeout');
 }
 const post=async(base,route,body)=>{
  const response=await fetch(base+route,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body),signal:AbortSignal.timeout(120000)});
  return {http:response.status,body:await response.json()};
 };
 const probe=async(...args)=>run(path.join(root,'target/release/examples/racing_service_control_probe'),args,{cwd:root,timeout:120000,maxBuffer:65536});
 const verificationRoute='/v1/verifications/racing/services';
 try{
  await fs.writeFile(common.PITGUN_SIGNING_SECRET_FILE,randomBytes(32).toString('hex'),{mode:0o600});
  const manifest=await write('retained.json',{schema:'pitgun.retained-service-replays/v1',replays:[{
   catalog_release_dir:catalog,node:process.execPath,script:path.join(root,'services/pitgun-verifier/wasm-service-replay.mjs'),
   wasm:oldWasm,bridge:oldBridge,runtime_digest:pins.legacy.wasm,bridge_digest:pins.legacy.bridge,timeout_seconds:120}]});
  const verifier=await launch('pitgun-verifier','PITGUN_VERIFIER_BIND',{PITGUN_RETAINED_SERVICE_REPLAYS_FILE:manifest});
  const authority=await launch('pitgun-authority','PITGUN_AUTHORITY_BIND');
  const oldAuthority=await launch('pitgun-authority','PITGUN_AUTHORITY_BIND',{PITGUN_RACING_SERVICE_WASM_DIGEST:pins.legacy.wasm});
  assert.equal((await fetch(verifier+verificationRoute+'/readyz')).status,200);
  const release=JSON.parse(await fs.readFile(path.join(catalog,'release.json'),'utf8'));
  const fixture=JSON.parse(await fs.readFile(path.join(root,'crates/pitgun-racing-simulator/fixtures/control/monza-five-laps.json'),'utf8'));
  const cases=[['candidate','keep','it-1922'],['candidate','pit','it-1922'],['candidate','timeout','it-1922'],
   ['candidate','keep','silverstone'],['candidate','keep','monaco'],['legacy','keep','it-1922']];
  let kept;
  for(const [index,[runtime,mode,track]] of cases.entries()){
   const name=runtime+'-'+mode+'-'+track,input=structuredClone(fixture.input);
   // Read the exact circuit ID from the immutable pack, never infer it from labels.
   input.track_id=track==='it-1922'?track:JSON.parse(await fs.readFile(path.join(catalog,'simulation/circuits',track+'.json'),'utf8')).meta.id;
   input.era=3;input.hz=5;input.laps=7;
   input.competitors[0].stint_strategy={stints:[{tire_id:'soft',laps:3},{tire_id:'medium',laps:2},{tire_id:'hard',laps:2}],pit_laps:[3,5]};
   const request={subject:'test.player',execution_id:'018f3b78-7e9a-7d20-a5e1-4ed92f02a59'+(index+1),seed:'7',input,catalog_release:release};
   const auth=await post(runtime==='legacy'?oldAuthority:authority,'/v1/authorizations/racing/services',request);
   assert.equal(auth.http,200,JSON.stringify(auth.body));
   const authPath=await write(name+'-authorization.json',auth.body),payloadPath=path.join(directory,name+'-payload.json');
   await probe('prepare',authPath,catalog,payloadPath);
   const payload=JSON.parse(await fs.readFile(payloadPath,'utf8')),config=JSON.parse(payload.configJson);
   config.signed_service={signed:auth.body.signed,decision_envelope:auth.body.decision_envelope,catalog_release:auth.body.catalog_release};
   payload.configJson=JSON.stringify(config);await write(name+'-payload.json',payload);
   const completionPath=path.join(directory,name+'-completion.json');
   await run(process.execPath,['--max-old-space-size=512',self,'--execute',payloadPath,runtime==='legacy'?oldWasm:wasm,runtime==='legacy'?oldBridge:bridge,mode,completionPath,runtime==='legacy'?'full':completionTransport],{timeout:120000,maxBuffer:65536,env:{PATH:process.env.PATH}});
   const completion=JSON.parse(await fs.readFile(completionPath,'utf8')),evidence=completion.service_evidence;
   const reply=await post(verifier,verificationRoute,evidence);
   assert.equal(reply.http,200,JSON.stringify(reply));assert.equal(reply.body.status,'VERIFIED',JSON.stringify(reply));
   assert.deepEqual(evidence.output.player_pit_laps,mode==='pit'?[2]:[3,5]);
   checks.push({case:name,status:reply.body.status,historyCount:evidence.applied_history.length,pitLaps:evidence.output.player_pit_laps,verifierDigest:reply.body.verifier.digest});
   if(index===0)kept={evidence,authPath,completionPath};
   console.log(JSON.stringify(checks.at(-1)));
  }
  assert.notEqual(checks[0].verifierDigest,checks.at(-1).verifierDigest,'Adapter/runtime must be bound into verifier identity');
  for(const kind of ['missing','extra','reordered','observation','receipt','late','tire']){
   const history=structuredClone(kept.evidence.applied_history);
   if(kind==='missing')history.shift();
   if(kind==='extra')history.push(history[0]);
   if(kind==='reordered')history.reverse();
   if(kind==='observation'){history[0].observation_digest='sha256:'+'f'.repeat(64);history[0].applied.proposal.observation_digest=history[0].observation_digest;}
   if(kind==='receipt')history[0].applied.pit_after_lap=2;
   if(kind==='late')history[0].applied.proposal.decision_at_us+=1000000000;
   if(kind==='tire')history[0].applied.proposal.action={kind:'pit_next_lap',tire_id:'undelegated'};
   const historyPath=await write(kind+'-history.json',history),proofPath=path.join(directory,kind+'-proof.json');
   await probe('evidence',kept.authPath,catalog,proofPath,kept.completionPath,historyPath);
   const reply=await post(verifier,verificationRoute,JSON.parse(await fs.readFile(proofPath,'utf8')));
   assert.equal(reply.body.status,'REJECTED',kind);assert.equal(reply.body.reason_code,'replay_mismatch',JSON.stringify(reply));
   checks.push({case:'forged-'+kind,status:reply.body.status,reason:reply.body.reason_code});
  }
  const bad=structuredClone(kept.evidence);
  bad.signed_authorization.signature=(bad.signed_authorization.signature[0]==='0'?'1':'0')+bad.signed_authorization.signature.slice(1);
  const unsigned=await post(verifier,verificationRoute,bad);
  assert.equal(unsigned.http,200);assert.equal(unsigned.body.status,'REJECTED');assert.equal(unsigned.body.reason_code,'invalid_authorization');
  checks.push({case:'invalid-signature',http:unsigned.http,status:unsigned.body.status,reason:unsigned.body.reason_code});
  const unknown=structuredClone(kept.evidence);unknown.decision_envelope.runtime_digest='sha256:'+'e'.repeat(64);
  const rejected=await post(verifier,verificationRoute,unknown);assert.equal(rejected.body.status,'REJECTED');
  checks.push({case:'unknown-runtime',status:rejected.body.status,reason:rejected.body.reason_code});
  // Corrupt only this recipe's disposable server copy, never either pinned source.
  const original=await fs.readFile(serverWasm),corrupt=Buffer.from(original);corrupt[corrupt.length-1]^=1;
  try{
   await fs.writeFile(serverWasm,corrupt);
   const pending=await post(verifier,verificationRoute,kept.evidence);
   assert.equal(pending.http,202);assert.equal(pending.body.reason_code,'dependency_unavailable');
   checks.push({case:'changed-server-artifact',status:pending.body.status,reason:pending.body.reason_code});
  }finally{await fs.writeFile(serverWasm,original);}
  const recovered=await post(verifier,verificationRoute,kept.evidence);
  assert.equal(recovered.body.status,'VERIFIED');
  checks.push({case:'artifact-restored',status:recovered.body.status});
  for(const kind of ['bad-retained-pin','duplicate-retained-pair']){
   const config=JSON.parse(await fs.readFile(manifest,'utf8'));
   if(kind==='bad-retained-pin')config.replays[0].bridge_digest='sha256:'+'d'.repeat(64);
   else config.replays.push(structuredClone(config.replays[0]));
   const file=await write(kind+'.json',config);
   let refusal;
   try{
    await run(path.join(root,'target/release/pitgun-verifier'),[],{cwd:root,timeout:30000,maxBuffer:65536,
     env:{...common,PITGUN_VERIFIER_BIND:'127.0.0.1:0',PITGUN_RETAINED_SERVICE_REPLAYS_FILE:file}});
   }catch(error){refusal=error;}
   assert.ok(refusal&&refusal.code===1,'Bad operator configuration must fail startup, not start or hang');
   checks.push({case:kind,status:'startup-refused'});
  }
  await write('report.json',{passed:true,completionTransport,scope:'Disposable local HTTP services; signed actual WASM executions; reference controller, no LLM; short fixtures, no career/publication or remote worker deployment',pins,checks});
 }catch(e){await write('report.json',{passed:false,error:String(e),pins,checks});throw e;}
 finally{
  for(const child of children){
   if(child.exitCode===null){child.kill('SIGTERM');await Promise.race([new Promise(r=>child.once('exit',r)),new Promise(r=>setTimeout(r,1000))]);if(child.exitCode===null)child.kill('SIGKILL');}
  }
  await fs.rm(temporary,{recursive:true,force:true});
 }
}
