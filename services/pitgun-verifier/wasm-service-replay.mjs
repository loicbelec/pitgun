// Trusted server adapter, not an endpoint or user-code runner. All file paths
// and expected hashes are operator-owned arguments; only bounded data is stdin.
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
const [wasmFile,bridgeFile,wasmDigest,bridgeDigest]=process.argv.slice(2);
const hash=b=>'sha256:'+createHash('sha256').update(b).digest('hex');
const stable=v=>JSON.stringify(sort(v));
function sort(v){if(Array.isArray(v))return v.map(sort);if(v&&typeof v==='object')return Object.fromEntries(Object.keys(v).sort().map(k=>[k,sort(v[k])]));return v;}
let code='runtime_unavailable';
try{
 const bytes=fs.readFileSync(wasmFile),bridge=fs.readFileSync(bridgeFile);
 if(hash(bytes)!==wasmDigest||hash(bridge)!==bridgeDigest)throw Error('Pinned runtime/bridge bytes differ');
 let size=0;const chunks=[];
 for await(const chunk of process.stdin){size+=chunk.length;if(size>32*1024*1024)throw Error('Replay input exceeds bound');chunks.push(chunk);}
 const input=JSON.parse(Buffer.concat(chunks).toString('utf8'));
 const api=await import(pathToFileURL(bridgeFile).href);await api.default({module_or_path:bytes});
 code='replay_mismatch';
 const parse=raw=>{const r=JSON.parse(raw);if(r.error)throw Error(r.error);return r;};
 const start=parse(api.start_experimental_controlled_racing_session_json(input.requestJson,input.catalogBundleJson,input.timelineJson,input.configJson));
 let index=0,output;
 for(let pulls=0;pulls<100000;pulls++){
  const next=parse(api.pull_experimental_controlled_racing_session_json(start.handle));
  if(next.kind==='decision'){
   const record=input.history[index++];
   if(!record||record.observation_digest!==next.observation_digest)throw Error('Regenerated observation digest differs');
   const result=parse(api.propose_experimental_controlled_racing_session_json(start.handle,JSON.stringify(record.applied.proposal)));
   if(stable(result.receipt)!==stable(record.applied))throw Error('Regenerated receipt differs');
  }else if(next.kind==='batch'&&next.complete){
   if(index!==input.history.length)throw Error('Extra service history');
   const completed=parse(api.complete_experimental_controlled_racing_session_json(start.handle));
   if(!completed.native_evidence){code='runtime_unavailable';throw Error('Runtime lacks compact native evidence');}
   output=completed.native_evidence;break;
  }else if(next.kind!=='batch')throw Error('Unexpected control response');
 }
 if(!output)throw Error('Replay pull bound exceeded');
 const encoded=JSON.stringify({ok:true,evidence:output});if(Buffer.byteLength(encoded)>65536){code='runtime_unavailable';throw Error('Compact replay evidence exceeds bound');}
 process.stdout.write(encoded);
}catch(error){process.stdout.write(JSON.stringify({ok:false,code,error:String(error.message).slice(0,160)}));}
