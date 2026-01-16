const fs = require("fs");
const path = require("path");

let WebSocket;
try {
  WebSocket = require("ws");
} catch (err) {
  console.error(
    "Missing dependency 'ws'. Install it with: npm install ws --prefix crates/pitgun-source-physics-wasm"
  );
  process.exit(1);
}

const dataDir = process.env.PITGUN_TELEMETRY_DATA_DIR;
if (!dataDir) {
  console.error(
    "Missing PITGUN_TELEMETRY_DATA_DIR. Point it at the telemetryd data directory."
  );
  process.exit(1);
}

const wasm = require("../pkg/pitgun_source_physics_wasm.js");
const SESSION_ID = "dev-session-1";
const WS_URL = "ws://127.0.0.1:8080/ws";

function telemetryToEvents(point) {
  const ts_ns = Math.round(point.time_s * 1e9).toString();
  return [
    { channel: "sim.time_s", ts_ns, value: point.time_s },
    { channel: "sim.s_m", ts_ns, value: point.s_m },
    { channel: "sim.x_m", ts_ns, value: point.x_m },
    { channel: "sim.y_m", ts_ns, value: point.y_m },
    { channel: "sim.heading_rad", ts_ns, value: point.heading_rad },
    { channel: "sim.speed_kph", ts_ns, value: point.speed_kph },
    { channel: "sim.rpm", ts_ns, value: point.rpm },
    { channel: "sim.gear", ts_ns, value: point.gear },
    { channel: "sim.throttle_pct", ts_ns, value: point.throttle_pct },
    { channel: "sim.brake_pct", ts_ns, value: point.brake_pct },
    { channel: "sim.g_lat", ts_ns, value: point.g_lat },
    { channel: "sim.g_long", ts_ns, value: point.g_long },
    { channel: "sim.engine_temp_c", ts_ns, value: point.engine_temp_c },
    { channel: "sim.engine_power_w", ts_ns, value: point.engine_power_w },
  ];
}

function buildEnvelope(events, endOfStream) {
  return {
    schema_version: 1,
    session_id: SESSION_ID,
    sent_at_ms: Date.now(),
    batch: {
      events,
      end_of_stream: endOfStream,
    },
  };
}

function sendJson(socket, payload) {
  return new Promise((resolve, reject) => {
    socket.send(JSON.stringify(payload), (err) => {
      if (err) {
        reject(err);
      } else {
        resolve();
      }
    });
  });
}

function waitForOpen(socket) {
  return new Promise((resolve, reject) => {
    socket.once("open", resolve);
    socket.once("error", reject);
  });
}

function closeSocket(socket) {
  return new Promise((resolve) => {
    if (socket.readyState === WebSocket.CLOSED) {
      resolve();
      return;
    }
    socket.once("close", resolve);
    socket.close();
  });
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function todayUtc() {
  return new Date().toISOString().slice(0, 10);
}

async function waitForNdjson(pathname, startedAtMs) {
  const attempts = 30;
  const delayMs = 200;
  const minReceivedAt = startedAtMs - 1000;
  let lastError;

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      const contents = fs.readFileSync(pathname, "utf8");
      const lines = contents.split(/\r?\n/).filter(Boolean);
      if (lines.length === 0) {
        throw new Error("NDJSON file has no lines yet");
      }

      let sessionLines = 0;
      let hasSpeed = false;
      let hasRpm = false;

      for (const line of lines) {
        const record = JSON.parse(line);
        if (record.session_id !== SESSION_ID) {
          continue;
        }
        if (
          typeof record.received_at_ms === "number" &&
          record.received_at_ms < minReceivedAt
        ) {
          continue;
        }

        sessionLines += 1;
        const events = (record.batch && record.batch.events) || [];
        for (const event of events) {
          if (event.channel === "sim.speed_kph") {
            hasSpeed = true;
          } else if (event.channel === "sim.rpm") {
            hasRpm = true;
          }
        }
      }

      if (sessionLines === 0) {
        throw new Error("No matching session lines yet");
      }
      if (!hasSpeed || !hasRpm) {
        throw new Error("Missing sim.speed_kph or sim.rpm events yet");
      }

      return { lines: lines.length, sessionLines };
    } catch (err) {
      lastError = err;
      await sleep(delayMs);
    }
  }

  throw lastError || new Error("Timed out waiting for NDJSON output");
}

async function run() {
  const startedAtMs = Date.now();

  const request = {
    track_id: "demo-oval",
    hz: 60,
    tuning: {
      aero_points: 10,
      chassis_points: 10,
      engine_points: 10,
      cooling_points: 10,
      downforce_slider: 0.5,
      gear_ratio_slider: 0.5,
    },
    seed: 1,
    engine_version: "0.1.0",
  };

  const resultJson = wasm.simulate(JSON.stringify(request));
  const result = JSON.parse(resultJson);

  const socket = new WebSocket(WS_URL);
  await waitForOpen(socket);

  const telemetry = result.telemetry || [];
  for (let i = 0; i < telemetry.length; i += 1) {
    const events = telemetryToEvents(telemetry[i]);
    const endOfStream = i === telemetry.length - 1;
    await sendJson(socket, buildEnvelope(events, endOfStream));
  }

  await closeSocket(socket);
  await sleep(200);

  const ndjsonPath = path.join(dataDir, `${todayUtc()}.ndjson`);
  const resultInfo = await waitForNdjson(ndjsonPath, startedAtMs);

  console.log("ok", {
    file: ndjsonPath,
    lines: resultInfo.lines,
    sessionLines: resultInfo.sessionLines,
  });
}

run().catch((err) => {
  console.error(err && err.stack ? err.stack : err);
  process.exit(1);
});
