#!/usr/bin/env node
import http from "node:http";

const model = "conduit-fixture:latest";
const digest = `sha256:${"f".repeat(64)}`;
const json = (response, status, value) => {
  const body = JSON.stringify(value);
  response.writeHead(status, { "content-type": "application/json", "content-length": Buffer.byteLength(body) });
  response.end(body);
};
const generate = prompt => {
  if (prompt.includes("Classify the following text")) return '{"label":"conduit"}';
  if (prompt.includes("Extract the subject")) return '{"subject":"Conduit"}';
  if (prompt.includes("Interpret this bounded operational evidence")) {
    return JSON.stringify({
      hypothesis: "The Line carrier was lost while a fresh Host offer remained available.",
      referenced_evidence: ["sign/line/carrier-lost/7", "sign/host/offer-fresh/9"],
      confidence_permille: 900,
      implications: ["A new Plan may select the fresh offer."],
      disposition: "interpreted",
    });
  }
  return "Conduit fixture response";
};

const server = http.createServer((request, response) => {
  const chunks = [];
  request.on("data", chunk => chunks.push(chunk));
  request.on("end", () => {
    let input = {};
    try { input = chunks.length ? JSON.parse(Buffer.concat(chunks)) : {}; }
    catch { return json(response, 400, { error: "invalid json" }); }
    if (request.url === "/api/version") return json(response, 200, { version: "fixture-http@1" });
    if (request.url === "/api/tags") return json(response, 200, { models: [{
      name: model, digest, size: 1048576,
      details: { family: "fixture", parameter_size: "finite", quantization_level: "exact" },
    }] });
    if (request.url === "/api/show") return json(response, 200, {
      model_info: { "general.architecture": "fixture", "fixture.context_length": 131072 },
      capabilities: ["completion"],
    });
    if (request.url === "/api/generate") return json(response, 200, {
      response: generate(input.prompt ?? ""), done: true, done_reason: "stop",
      prompt_eval_count: 8, eval_count: 8,
    });
    if (request.url === "/api/chat") {
      const schema = input.format?.properties?.suggested_action_identities?.items;
      const actions = Array.isArray(schema?.enum) ? schema.enum.slice(0, 1) : [];
      return json(response, 200, { message: { role: "assistant", content: JSON.stringify({
        speech: "I am lulled with my history retained.",
        presented_thought: "The Body can wake again from retained meaning.",
        suggested_action_identities: actions,
      }) }, done: true, done_reason: "stop", prompt_eval_count: 8, eval_count: 8 });
    }
    return json(response, 404, { error: "unsupported fixture endpoint" });
  });
});

const port = Number.parseInt(process.env.CONDUIT_FIXTURE_OLLAMA_PORT ?? "11434", 10);
server.listen(port, "127.0.0.1", () => process.stdout.write("fixture Ollama HTTP service ready\n"));
