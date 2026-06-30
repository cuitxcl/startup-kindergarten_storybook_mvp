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

  window.KindleleafApi = {
    baseUrl: apiBase,
    getDashboard: () => request("/api/dashboard/teacher"),
    listContentItems: () => request("/api/content-items?page_size=20"),
    listChildren: () => request("/api/children?page_size=50"),
    listCases: () => request("/api/cases?page_size=50"),
    listStorybooks: () => request("/api/storybooks?page_size=20"),
    listPages: (storybookId) => request(`/api/storybooks/${storybookId}/pages`),
  };
})();
