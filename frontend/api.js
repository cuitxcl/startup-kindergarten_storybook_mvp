(function () {
  const params = new URLSearchParams(window.location.search);
  const configuredBase = params.get("api") || window.KINDLEAF_API_BASE || "";
  const apiBase = configuredBase.replace(/\/$/, "");

  async function request(path, options = {}) {
    const response = await fetch(`${apiBase}${path}`, {
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
        ...(options.headers || {}),
      },
      ...options,
    });

    const text = await response.text();
    const data = text ? JSON.parse(text) : null;

    if (!response.ok) {
      const message = data?.error?.message || `请求失败：${response.status}`;
      throw new Error(message);
    }

    return data;
  }

  function json(method, body, headers = {}) {
    return {
      method,
      body: JSON.stringify(body),
      headers: {
        "Content-Type": "application/json",
        ...headers,
      },
    };
  }

  function idempotencyHeaders(prefix) {
    const random = globalThis.crypto?.randomUUID ? globalThis.crypto.randomUUID() : `${Date.now()}-${Math.random()}`;
    return { "Idempotency-Key": `${prefix}-${random}` };
  }

  window.KindleleafApi = {
    baseUrl: apiBase,
    getDashboard: () => request("/api/dashboard/teacher"),
    listContentItems: () => request("/api/content-items?page_size=20"),
    listChildren: () => request("/api/children?page_size=50"),
    listCases: () => request("/api/cases?page_size=50"),
    listCasesByTheme: (theme) => request(`/api/cases?page_size=20&theme=${encodeURIComponent(theme)}`),
    listStorybooks: () => request("/api/storybooks?page_size=20"),
    listPages: (storybookId) => request(`/api/storybooks/${storybookId}/pages`),
    generateStorybook: (payload) => request("/api/storybooks/generate", json("POST", payload)),
    updateStorybook: (storybookId, payload) => request(`/api/storybooks/${storybookId}`, json("PATCH", payload)),
    addPage: (storybookId, payload) => request(`/api/storybooks/${storybookId}/pages`, json("POST", payload)),
    updatePage: (storybookId, pageId, payload) => request(`/api/storybooks/${storybookId}/pages/${pageId}`, json("PATCH", payload)),
    deletePage: (storybookId, pageId) => request(`/api/storybooks/${storybookId}/pages/${pageId}`, { method: "DELETE" }),
    rewritePage: (storybookId, pageId, payload = {}) => request(`/api/storybooks/${storybookId}/pages/${pageId}/rewrite`, json("POST", payload)),
    createExport: (storybookId, payload) =>
      request(`/api/storybooks/${storybookId}/exports`, json("POST", payload, idempotencyHeaders("export"))),
    createShareLink: (storybookId, payload) =>
      request(`/api/storybooks/${storybookId}/share-links`, json("POST", payload, idempotencyHeaders("share"))),
    createPageImageTask: (pageId, payload) =>
      request(`/api/storybook-pages/${pageId}/image-tasks`, json("POST", payload, idempotencyHeaders("page-image"))),
    createStorybookImageTask: (storybookId, payload) =>
      request(`/api/storybooks/${storybookId}/image-tasks`, json("POST", payload, idempotencyHeaders("storybook-image"))),
    createChild: (payload) => request("/api/children", json("POST", payload)),
    updateChild: (childId, payload) => request(`/api/children/${childId}`, json("PATCH", payload)),
    listStorybookRoles: (storybookId) => request(`/api/storybooks/${storybookId}/roles`),
    updateStorybookRole: (storybookId, roleKey, payload) =>
      request(`/api/storybooks/${storybookId}/roles/${encodeURIComponent(roleKey)}`, json("PATCH", payload)),
  };
})();
