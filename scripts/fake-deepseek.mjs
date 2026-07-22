#!/usr/bin/env node
import http from "node:http";

const port = Number(process.argv[2] || process.env.PORT || 18082);
const mode = process.env.FAKE_DEEPSEEK_MODE || "ok";
const requireRedactedCustomization = process.env.FAKE_DEEPSEEK_REQUIRE_REDACTED_CUSTOMIZATION === "true";

function jsonResponse(res, status, payload) {
  const body = JSON.stringify(payload);
  res.writeHead(status, {
    "Content-Type": "application/json",
    "Content-Length": Buffer.byteLength(body),
  });
  res.end(body);
}

function responseContentFor(requestBody) {
  const text = JSON.stringify(requestBody);
  if (mode === "wrong_shape") {
    return { message: "fake wrong shape" };
  }

  if (mode === "sensitive_output") {
    return {
      pages: [
        {
          page_number: 1,
          title: "敏感输出验证",
          body: "老师说，家长手机号 138 0013 8000 不能进入绘本正文。",
          illustration_prompt: "幼儿园教室里，老师和孩子一起读绘本",
          status: "draft",
        },
      ],
      editor_notes: ["这条输出应被后端内容安全拦截"],
    };
  }

  if (text.includes("storybook_roles")) {
    return {
      roles: [
        {
          name: "真真",
          role_type: "protagonist",
          appearance: "短发、蓝色外套、表情认真",
          story_function: "代表正在学习规则的孩子",
          needs_consistency: true,
        },
        {
          name: "周老师",
          role_type: "teacher",
          appearance: "温和、稳定、穿浅色围裙",
          story_function: "把规则解释成孩子能做的小步骤",
          needs_consistency: true,
        },
      ],
      consistency_guide: ["固定服装主色", "老师每页保持同一发型和围裙"],
    };
  }

  if (text.includes("storybook_pages")) {
    return {
      pages: [
        {
          page_number: 1,
          title: "水龙头前排好队",
          body: "真真来到洗手台前，看到朋友们一个一个等着洗手。",
          illustration_prompt: "幼儿园洗手区，孩子们排队等待，老师在旁边微笑引导",
          status: "draft",
        },
        {
          page_number: 2,
          title: "轮到我再伸手",
          body: "周老师说，等前面的朋友洗完，我们再轻轻伸出小手。",
          illustration_prompt: "老师蹲下和孩子平视，孩子看着洗手台等待",
          status: "draft",
        },
      ],
      editor_notes: ["文字适合共读", "插图提示保留排队和洗手动作"],
    };
  }

  if (text.includes("customization_plan")) {
    return {
      customization: {
        child_id: "fake-child-id",
        intensity: "standard",
        strategy: "保留洗手排队主线，替换孩子称呼和兴趣道具。",
        rewrite_points: [
          { scope: "title", action: "加入孩子昵称" },
          { scope: "pages", action: "把鼓励语改成孩子熟悉的表达" },
        ],
        risk_checks: ["不写入家庭住址", "不暴露敏感健康信息"],
      },
    };
  }

  return {
    plan: {
      title: "排队洗手小约定",
      theme: "排队洗手",
      age_group: "4-5 岁",
      summary: "孩子们在老师引导下学习排队、等待和洗手步骤。",
      page_count: 6,
      outline: [
        {
          page_range: "1",
          goal: "进入场景",
          beat: "孩子们来到洗手台前，发现大家都想先洗手。",
        },
        {
          page_range: "2-3",
          goal: "理解规则",
          beat: "老师把排队等待解释成可以练习的小约定。",
        },
      ],
      role_requirements: ["主角儿童", "老师引导者", "同伴儿童"],
      review_points: ["教学目标是否准确", "语言是否适合班级共读"],
    },
  };
}

const server = http.createServer((req, res) => {
  if (req.method === "GET" && req.url === "/health") {
    jsonResponse(res, 200, { status: "ok" });
    return;
  }

  if (req.method !== "POST" || req.url !== "/chat/completions") {
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

    if (mode === "http_500") {
      jsonResponse(res, 500, { error: "fake_provider_failure", retryable: true });
      return;
    }

    let requestBody;
    try {
      requestBody = JSON.parse(raw);
    } catch {
      jsonResponse(res, 400, { error: "invalid_json" });
      return;
    }

    if (requireRedactedCustomization && !customizationPromptIsRedacted(requestBody)) {
      jsonResponse(res, 400, {
        error: "customization_prompt_not_redacted",
      });
      return;
    }

    if (mode === "invalid_content") {
      jsonResponse(res, 200, {
        id: "fake-deepseek-chatcmpl-invalid",
        object: "chat.completion",
        choices: [{ index: 0, message: { role: "assistant", content: "not json" } }],
        usage: {
          prompt_tokens: 12,
          completion_tokens: 8,
          total_tokens: 20,
        },
      });
      return;
    }

    const content = JSON.stringify(responseContentFor(requestBody));
    jsonResponse(res, 200, {
      id: "fake-deepseek-chatcmpl",
      object: "chat.completion",
      choices: [{ index: 0, message: { role: "assistant", content } }],
      usage: {
        prompt_tokens: 120,
        completion_tokens: 80,
        total_tokens: 200,
      },
    });
  });
});

function customizationPromptIsRedacted(requestBody) {
  const text = JSON.stringify(requestBody);
  if (!text.includes("customization_plan")) {
    return true;
  }
  return (
    text.includes("[redacted]") &&
    !text.includes("Provider Smoke 儿童") &&
    !text.includes("parent@example.com") &&
    !text.includes("138 0013 8000") &&
    !text.includes("爸爸近期出差")
  );
}

server.listen(port, "127.0.0.1", () => {
  console.log(`fake deepseek listening on http://127.0.0.1:${port} mode=${mode}`);
});
