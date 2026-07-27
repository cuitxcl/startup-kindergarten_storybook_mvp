#!/usr/bin/env node
import http from "node:http";

const port = Number(process.argv[2] || process.env.PORT || 18183);
const mode = process.env.FAKE_SEEDREAM_MODE || "ok";
const requireRedactedPrompt = process.env.FAKE_SEEDREAM_REQUIRE_REDACTED_PROMPT === "true";
const requireValidPayload = process.env.FAKE_SEEDREAM_REQUIRE_VALID_PAYLOAD === "true";
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

    if (requireValidPayload) {
      const payloadError = validateSeedreamPayload(payload);
      if (payloadError) {
        jsonResponse(res, 400, payloadError);
        return;
      }
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

function validateSeedreamPayload(payload) {
  if (typeof payload.model !== "string" || payload.model.trim() === "") {
    return { error: "missing_model" };
  }
  if (typeof payload.prompt !== "string" || payload.prompt.trim() === "") {
    return { error: "missing_prompt" };
  }
  if (typeof payload.size !== "string" || !/^\d+x\d+$/.test(payload.size)) {
    return { error: "invalid_size", size: payload.size };
  }
  if (payload.response_format !== "b64_json") {
    return { error: "invalid_response_format", response_format: payload.response_format };
  }
  if (payload.output_format !== "png") {
    return { error: "invalid_output_format", output_format: payload.output_format };
  }
  if (payload.watermark !== false) {
    return { error: "invalid_watermark", watermark: payload.watermark };
  }
  if (!["text_to_image", "reference_image", "edit_image"].includes(payload.image_mode)) {
    return { error: "invalid_image_mode", image_mode: payload.image_mode };
  }
  if (payload.image !== undefined && !Array.isArray(payload.image)) {
    return { error: "invalid_image_references" };
  }
  if (payload.reference_images !== undefined && !Array.isArray(payload.reference_images)) {
    return { error: "invalid_reference_images" };
  }
  if (payload.strength !== undefined) {
    const strength = Number(payload.strength);
    if (!Number.isFinite(strength) || strength < 0 || strength > 1) {
      return { error: "invalid_strength", strength: payload.strength };
    }
  }
  return null;
}

server.listen(port, "127.0.0.1", () => {
  console.log(`fake seedream image listening on http://127.0.0.1:${port} mode=${mode}`);
});
