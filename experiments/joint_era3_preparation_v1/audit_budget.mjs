// Pure dependency/point audit. No browser, storage, service or race calculation.
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import {resolve,join} from 'node:path';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const [gameArg,out]=process.argv.slice(2);if(!out)throw Error('GAME OUTPUT required');const game=resolve(gameArg);
if(execFileSync('git',['status','--porcelain'],{cwd:game,encoding:'utf8'}).trim())throw Error('Pin clean game source');
const require=createRequire(join(game,'package.json'));const{register}=await import(pathToFileURL(require.resolve('tsx/esm/api')));register({tsconfig:join(game,'tsconfig.app.json')});
const load=p=>import(pathToFileURL(join(game,p)));const{SEASON_DEVELOPMENTS}=await load('src/economy/seasonContent.ts');const{seasonDevelopmentPoints}=await load('src/economy/seasonCareer.ts');const{getUnlockedCarSetupOffsets}=await load('src/engine/carSetup.ts');
const byId=new Map(SEASON_DEVELOPMENTS.map(d=>[d.id,d]));const starting=['e1_desk','e1_toolbox'];
function close(set){
 const next=new Set(set);
 for(const id of next){const d=byId.get(id);if(!d){if(!starting.includes(id))throw Error('Unknown '+id);continue;}for(const dep of d.requires)next.add(dep);}
 for(const id of next){const d=byId.get(id);if(d?.requiresAny&&!d.requiresAny.some(dep=>next.has(dep)))return d.requiresAny.flatMap(dep=>close(new Set([...next,dep])));}
 return[next];
}
const all=close(new Set([...starting,'e1_industrialization','e2_mainframe']));const unique=[...new Map(all.map(s=>[[...s].sort().join(','),s])).values()];const minimal=unique.filter(s=>!unique.some(t=>t.size<s.size&&[...t].every(id=>s.has(id))));
const foundations=minimal.map(s=>{const upgrades=[...s].sort();const offsets=getUnlockedCarSetupOffsets({upgrades});const discretionaryPoints=seasonDevelopmentPoints(upgrades);return{upgrades,discretionaryPoints,offsets,effectiveCap:discretionaryPoints+Object.values(offsets).reduce((a,b)=>a+b,0)};});
const files=['src/economy/seasonContent.ts','src/economy/seasonCatalogue.ts','src/economy/seasonCareer.ts','src/engine/carSetup.ts','src/data/Upgrades.ts'];const hashes=Object.fromEntries(await Promise.all(files.map(async p=>[p,createHash('sha256').update(await readFile(join(game,p))).digest('hex')])));
const result={schema:'pitgun.era3-structural-budget-audit/v1',gameCommit:execFileSync('git',['rev-parse','HEAD'],{cwd:game,encoding:'utf8'}).trim(),sourceHashes:hashes,foundations,minimumFoundationEffectiveCap:Math.min(...foundations.map(f=>f.effectiveCap)),scope:'Dependency closure of permanent era-entry milestones using public preparation/offset functions. No immediate affordability or human play-duration claim. Existing ownership can only add nonnegative point/offset effects; optional ceiling is separately exported.'};
await writeFile(resolve(out),JSON.stringify(result,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({foundations:foundations.length,minimumEffectiveCap:result.minimumFoundationEffectiveCap}));
