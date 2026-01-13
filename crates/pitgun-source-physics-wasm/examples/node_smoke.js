const fs = require("fs");
const path = require("path");

const signingSecret = process.env.PITGUN_SIGNING_SECRET;
if (!signingSecret) {
  console.error(
    "Missing PITGUN_SIGNING_SECRET. Set it to the signing secret used for the fixture."
  );
  process.exit(1);
}

const contractPath = path.join(
  __dirname,
  "..",
  "tests",
  "fixtures",
  "sim_contract.json"
);
const contractJson = fs.readFileSync(contractPath, "utf8");

const wasm = require("../pkg/pitgun_source_physics_wasm.js");

const sim = wasm.PhysicsWasm.new_with_secret(contractJson, signingSecret);
const batch = sim.next_batch();

if (!batch || !Array.isArray(batch.events)) {
  throw new Error("expected events array in batch");
}
if (batch.events.length === 0) {
  throw new Error("expected non-empty events");
}

const channels = new Set(batch.events.map((event) => event.channel));
if (!channels.has("speed_kph") || !channels.has("rpm")) {
  throw new Error("expected speed_kph and rpm channels");
}

console.log("ok", {
  end_of_stream: batch.end_of_stream,
  events: batch.events.length,
});
