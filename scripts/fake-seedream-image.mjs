#!/usr/bin/env node
import http from "node:http";

const port = Number(process.argv[2] || process.env.PORT || 18183);
const mode = process.env.FAKE_SEEDREAM_MODE || "ok";
const requireRedactedPrompt = process.env.FAKE_SEEDREAM_REQUIRE_REDACTED_PROMPT === "true";
const transparentPngBase64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGP4cGnfsxNbGCAUAEWMCcWN1afmAAAAAElFTkSuQmCC";

function jsonResponse(res, status, payload) {
  const body = JSON.stringify(payload);
  res.writeHead(status, {
    "Content-Type": "application/json",
    "Content-Length": Buffer.byteLength(body),
  });
  res.end(body);
}

const server = http.createServer((req, res) => {
  if (req.method === "GET" && req.url === "/health") {
    jsonResponse(res, 200, { status: "ok" });
    return;
  }

  if (req.method !== "POST" || req.url !== "/api/v3/images/generations") {
    jsonResponse(res, 404, { error: "not_found" });
    return;
  }

  let raw = "";
  req.setEncoding("utf8");
  req.on("data", (chunk) => {
    raw += chunk;
  });
  req.on("end", () => {
    const auth = req.headers.authorization || "";
    if (!auth.startsWith("Bearer ")) {
      jsonResponse(res, 401, { error: "missing_bearer" });
      return;
    }

    let payload;
    try {
      payload = JSON.parse(raw);
    } catch {
      jsonResponse(res, 400, { error: "invalid_json" });
      return;
    }

    if (requireRedactedPrompt && !promptIsRedacted(payload.prompt || "")) {
      jsonResponse(res, 400, {
        error: "prompt_not_redacted",
        prompt: payload.prompt || "",
      });
      return;
    }

    if (mode === "http_500") {
      jsonResponse(res, 500, { error: "fake_seedream_failure", retryable: true });
      return;
    }

    jsonResponse(res, 200, {
      created: Math.floor(Date.now() / 1000),
      data: [
        {
          b64_json:
            mode === "invalid_png"
              ? Buffer.from("not-a-png", "utf8").toString("base64")
              : transparentPngBase64,
        },
      ],
    });
  });
});

function promptIsRedacted(prompt) {
  return (
    prompt.includes("[phone_redacted]") &&
    prompt.includes("[email_redacted]") &&
    prompt.includes("[private_detail_redacted]") &&
    !prompt.includes("138 0013 8000") &&
    !prompt.includes("parent@example.com") &&
    !prompt.includes("家长电话")
  );
}

server.listen(port, "127.0.0.1", () => {
  console.log(`fake seedream image listening on http://127.0.0.1:${port} mode=${mode}`);
});
