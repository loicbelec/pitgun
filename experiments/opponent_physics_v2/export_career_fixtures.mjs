/** Read-only real-game reducer/preparation export. No browser, storage or network.
 * Usage: node export_career_fixtures.mjs GAME_CHECKOUT CATALOG_DIRECTORY OUTPUT
 * Accounting results below are authored fixtures, never real race performance.
 */
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';
import { readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve, relative, join } from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const [gameArg, catalogArg, outputArg] = process.argv.slice(2);
if (!outputArg) throw new Error('Require GAME_CHECKOUT CATALOG_DIRECTORY OUTPUT');
const game = resolve(gameArg), catalogDir = resolve(catalogArg);
if (execFileSync('git', ['status', '--porcelain'], { cwd: game, encoding: 'utf8' }).trim()) throw new Error('Pin a clean game checkout');
const require = createRequire(join(game, 'package.json'));
const { register } = await import(pathToFileURL(require.resolve('tsx/esm/api')));
const imported = new Set();
register({ tsconfig: join(game, 'tsconfig.app.json'), onImport(url) {
  if (url.startsWith('file:')) {
    const path = fileURLToPath(url.split('?')[0]);
    if (path.startsWith(game + '/') && !path.includes('/node_modules/')) imported.add(path);
  }
} });
const load = path => import(pathToFileURL(join(game, path)));
const career = await load('src/economy/seasonCareer.ts');
const { currentSeasonId } = await load('src/economy/seasonProgression.ts');
const { PLAYABLE_SEASONS } = await load('src/economy/seasonContent.ts');
const { start, enter, finish, circuits } = await load('src/economy/seasonTestFixtures.ts');
const { PUBLIC_SEASON_ROUTE } = await load('scripts/economy/public-season-route.ts');
const { seasonVehicleTechnology } = await load('src/economy/seasonVehicle.ts');
const { getUnlockedCarSetupOffsets } = await load('src/engine/carSetup.ts');
const { preparePitWallSession } = await load('src/app/pitwallSessionExecution.ts');
const { parseRacingOpponentPolicy } = await load('src/engine/opponentPolicy.ts');
const read = async path => JSON.parse(await readFile(path, 'utf8'));
const policy = parseRacingOpponentPolicy(await read(join(catalogDir, 'simulation/policies/competitive.json')));
const presentation = await read(join(catalogDir, 'presentation/index.json'));
const circuitInfo = new Map(presentation.circuits.map(c => [c.id, c]));
const driverFiles = (await readdir(join(catalogDir, 'simulation/drivers'))).sort();
const drivers = await Promise.all(driverFiles.filter(f => f.endsWith('.json')).map(async file => {
  const d = await read(join(catalogDir, 'simulation/drivers', file));
  return { id: d.id ?? file.slice(0, -5), display_name: d.display_name ?? file.slice(0, -5) };
}));
const hash = bytes => 'sha256:' + createHash('sha256').update(bytes).digest('hex');
const stateDigest = state => hash(JSON.stringify(state));
const fixtures = [];
const command = career.applySeasonCommand;

function capture(state, stage, ledger) {
  const technology = seasonVehicleTechnology(state.era, state.upgrades, true);
  const offsets = getUnlockedCarSetupOffsets(state);
  const points = career.seasonDevelopmentPoints(state.upgrades);
  const plan = PLAYABLE_SEASONS[state.seasonIndex];
  const downforceAvailable = technology.components?.aero_id === 'basic';
  const setups = Object.fromEntries(['aero', 'chassis', 'engine', 'cooling'].map(k => [k, Math.floor(points / 4)]));
  setups.engine += points - Object.values(setups).reduce((a, b) => a + b, 0);
  const requests = plan.calendar.map((circuitId, index) => {
    const info = circuitInfo.get(circuitId);
    if (!info?.laps) throw new Error(`Missing pinned GP distance ${circuitId}`);
    const prepareInput = { sessionType: 'RACE', executionId: `physics:${state.doctrine}:${stage}:${circuitId}`, weekendId: `physics:${stage}`,
      setup: { ...setups, downforce: 0.5, gearRatio: 0.5 }, setupOffsets: offsets, strategy: [], laps: info.laps,
      trackId: circuitId, vehicleId: technology.vehicleId, vehicleComponents: technology.components,
      componentCompositionAvailable: true, downforceAvailable, seed: 17041 + index, samplingHz: 5, era: state.era,
      playerBudget: points, availableDrivers: drivers, tireCatalog: { soft: 'soft', medium: 'medium', hard: 'hard' }, competitorCount: 9, opponentPolicy: policy };
    const prepared = preparePitWallSession(prepareInput);
    const concentrated = preparePitWallSession({ ...prepareInput, setup: { aero: 0, cooling: 0, engine: points, chassis: 0, downforce: 0.5, gearRatio: 0.5 } });
    if (JSON.stringify(prepared.request.competitors.slice(1)) !== JSON.stringify(concentrated.request.competitors.slice(1))) throw new Error('Player setup changed the opponent field');
    if (prepared.request.competitors.length !== 10) throw new Error('Expected nine rivals');
    return { circuitId, request: { input: prepared.request, seed: prepared.request.seed, era: state.era, hz: 5 } };
  });
  fixtures.push({ id: `${state.doctrine}:${stage}`, era: state.era, stage, doctrine: state.doctrine,
    careerStateDigest: stateDigest(state), purchases: state.purchases, upgrades: state.upgrades, budget: state.budget,
    discretionaryPoints: points, offsets, technology, sliders: { downforce: downforceAvailable ? [0, 1] : [0.5, 0.5], gearRatio: [0, 1] },
    calendar: plan.calendar, accountingLedger: [...ledger], preparedRequests: requests });
}

for (const doctrine of ['craft_engineering', 'industrial_discipline', 'commercial_push']) {
  let state = command(start(`physics-corpus:${doctrine}`), { kind: 'doctrine', id: doctrine });
  const ledger = [{ kind: 'doctrine', id: doctrine }];
  const buy = id => {
    if (!career.seasonPurchaseProblem(state, id)) { state = command(state, { kind: 'buy', id }); ledger.push({ kind: 'buy', id }); }
  };
  const season = async index => {
    for (let round = 0; round < state.rounds.length; round++) {
      for (const id of PUBLIC_SEASON_ROUTE.slice(0, index + 1).flat()) buy(id);
      state = enter(state);
      for (let session = 0; session < 4; session++) state = await finish(state);
      ledger.push({ kind: 'authored-accounting-weekend', season: currentSeasonId(state), circuit: state.active.circuitId, sessionLaps: 2, fixturePosition: 9 });
      state = command(state, { kind: 'leave' });
    }
  };
  capture(state, 'era1-entry', ledger);
  await season(0); capture(state, 'era1-exit', ledger);
  buy('e1_industrialization');
  if (!state.upgrades.includes('e1_industrialization')) throw new Error('Authored route cannot qualify eraII');
  state = command(state, { kind: 'advance', seasonId: currentSeasonId(state), circuits });
  capture(state, 'era2-entry', ledger);
  await season(1);
  state = command(state, { kind: 'advance', seasonId: currentSeasonId(state), circuits });
  await season(2); capture(state, 'era2-exit', ledger);
}
const sourceFiles = Object.fromEntries(await Promise.all([...imported].sort().map(async file => [relative(game, file), hash(await readFile(file))])));
const payload = { schema: 'pitgun.real-career-physics-fixtures/v1', gameCommit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: game, encoding: 'utf8' }).trim(),
  gameSourceFiles: sourceFiles, exporterDigest: hash(await readFile(fileURLToPath(import.meta.url))),
  catalogDigest: hash(await readFile(join(catalogDir, 'catalog.json'))), fixtures,
  scope: 'Authored bookkeeping through real career reducers and session preparation. Fixed two-lap P9 accounting fixtures earn announced rewards; this does not prove pace, wins, real play duration or physical feasibility. Full GP requests use pinned catalog distances; no personal storage or player data.' };
const output = JSON.stringify(payload, null, 2) + '\n';
await writeFile(resolve(outputArg), output, { flag: 'wx' });
console.log(JSON.stringify({ fixtures: fixtures.length, preparedRequests: fixtures.reduce((n, f) => n + f.preparedRequests.length, 0), digest: hash(output) }));
