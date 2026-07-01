(function () {
  const params = new URLSearchParams(window.location.search);
  const configuredBase = params.get("api") || window.KINDLEAF_API_BASE || "";
  const apiBase = configuredBase.replace(/\/$/, "");
  const tokenStorageKey = "kindleaf_access_token";

  async function request(path, options = {}) {
    const token = currentToken();
    const { headers: optionHeaders = {}, ...fetchOptions } = options;
    const response = await fetch(`${apiBase}${path}`, {
      ...fetchOptions,
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
        ...optionHeaders,
      },
    });

    const text = await response.text();
    const data = text ? JSON.parse(text) : null;

    if (!response.ok) {
      const message = data?.error?.message || `请求失败：${response.status}`;
      const error = new Error(message);
      error.status = response.status;
      error.code = data?.error?.code;
      if (response.status === 401) {
        clearToken();
      }
      throw error;
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
    currentToken,
    setToken,
    clearToken,
    login: async (payload) => {
      const response = await request("/api/auth/login", json("POST", payload));
      setToken(response.access_token);
      return response;
    },
    me: () => request("/api/auth/me"),
    refreshSession: async () => {
      const response = await request("/api/auth/refresh", json("POST", {}));
      setToken(response.access_token);
      return response;
    },
    logout: async () => {
      try {
        return await request("/api/auth/logout", json("POST", {}));
      } finally {
        clearToken();
      }
    },
    getDashboard: () => request("/api/dashboard/teacher"),
    listContentItems: () => request("/api/content-items?page_size=20"),
    listContentItemActivity: (storybookId) => request(`/api/content-items/${storybookId}/activity`),
    getCurrentSchool: () => request("/api/schools/current"),
    updateCurrentSchool: (payload) => request("/api/schools/current", json("PATCH", payload)),
    listClassrooms: (params = {}) => request(`/api/classrooms${queryString(params)}`),
    createClassroom: (payload) => request("/api/classrooms", json("POST", payload)),
    updateClassroom: (classroomId, payload) => request(`/api/classrooms/${classroomId}`, json("PATCH", payload)),
    getCurrentTeacher: () => request("/api/teachers/me"),
    listTeachers: (params = {}) => request(`/api/teachers${queryString(params)}`),
    listChildren: () => request("/api/children?page_size=50"),
    getChild: (childId) => request(`/api/children/${childId}`),
    listCases: () => request("/api/cases?page_size=50"),
    listCasesByTheme: (theme) => request(`/api/cases?page_size=20&theme=${encodeURIComponent(theme)}`),
    getCase: (caseId) => request(`/api/cases/${caseId}`),
    cloneCase: (caseId, payload) => request(`/api/cases/${caseId}/clone`, json("POST", payload)),
    listTemplates: (params = {}) => request(`/api/story-templates${queryString(params)}`),
    getTemplate: (templateId) => request(`/api/story-templates/${templateId}`),
    createTemplate: (payload) => request("/api/story-templates", json("POST", payload)),
    updateTemplate: (templateId, payload) => request(`/api/story-templates/${templateId}`, json("PATCH", payload)),
    listStorybooks: (params = {}) => request(`/api/storybooks${queryString({ page_size: "20", ...params })}`),
    getStorybook: (storybookId) => request(`/api/storybooks/${storybookId}`),
    listPages: (storybookId) => request(`/api/storybooks/${storybookId}/pages`),
    generateStorybook: (payload) => request("/api/storybooks/generate", json("POST", payload)),
    updateStorybook: (storybookId, payload) => request(`/api/storybooks/${storybookId}`, json("PATCH", payload)),
    duplicateStorybook: (storybookId, payload = {}) => request(`/api/storybooks/${storybookId}/duplicate`, json("POST", payload)),
    deriveCustomStorybook: (storybookId, payload) => request(`/api/storybooks/${storybookId}/derive-custom`, json("POST", payload)),
    addPage: (storybookId, payload) => request(`/api/storybooks/${storybookId}/pages`, json("POST", payload)),
    updatePage: (storybookId, pageId, payload) => request(`/api/storybooks/${storybookId}/pages/${pageId}`, json("PATCH", payload)),
    deletePage: (storybookId, pageId) => request(`/api/storybooks/${storybookId}/pages/${pageId}`, { method: "DELETE" }),
    rewritePage: (storybookId, pageId, payload = {}) => request(`/api/storybooks/${storybookId}/pages/${pageId}/rewrite`, json("POST", payload)),
    createExport: (storybookId, payload) =>
      request(`/api/storybooks/${storybookId}/exports`, json("POST", payload, idempotencyHeaders("export"))),
    listExports: (storybookId) => request(`/api/storybooks/${storybookId}/exports`),
    getExport: (exportId) => request(`/api/exports/${exportId}`),
    createShareLink: (storybookId, payload) =>
      request(`/api/storybooks/${storybookId}/share-links`, json("POST", payload, idempotencyHeaders("share"))),
    listShareLinks: (storybookId) => request(`/api/storybooks/${storybookId}/share-links`),
    updateShareLink: (shareLinkId, payload) => request(`/api/share-links/${shareLinkId}`, json("PATCH", payload)),
    listSharedLibrary: (params = {}) => {
      const query = new URLSearchParams({ page_size: "20" });
      Object.entries(params).forEach(([key, value]) => {
        if (value !== undefined && value !== null && value !== "") {
          query.set(key, value);
        }
      });
      return request(`/api/shared-library?${query.toString()}`);
    },
    cloneSharedStorybook: (storybookId, payload) => request(`/api/shared-library/${storybookId}/clone`, json("POST", payload)),
    submitPlatformReview: (storybookId) => request(`/api/storybooks/${storybookId}/submit-platform-review`, json("POST", {})),
    createPageImageTask: (pageId, payload) =>
      request(`/api/storybook-pages/${pageId}/image-tasks`, json("POST", payload, idempotencyHeaders("page-image"))),
    createStorybookImageTask: (storybookId, payload) =>
      request(`/api/storybooks/${storybookId}/image-tasks`, json("POST", payload, idempotencyHeaders("storybook-image"))),
    getImageTask: (taskId) => request(`/api/image-tasks/${taskId}`),
    listReviewEvents: (taskId) => request(`/api/image-tasks/${taskId}/review-events`),
    retryImageTask: (taskId, payload) => request(`/api/image-tasks/${taskId}/retry`, json("POST", payload)),
    selectImageOutput: (outputId) => request(`/api/image-outputs/${outputId}/select`, json("POST", {})),
    reviewImageOutput: (outputId, payload) => request(`/api/image-outputs/${outputId}/review`, json("POST", payload)),
    listGenerationCosts: (storybookId) => request(`/api/admin/generation-costs${storybookId ? `?storybook_id=${storybookId}` : ""}`),
    createUploadIntent: (payload) => request("/api/assets/upload-intents", json("POST", payload)),
    createAsset: (payload) => request("/api/assets", json("POST", payload)),
    getAsset: (assetId) => request(`/api/assets/${assetId}`),
    createChild: (payload) => request("/api/children", json("POST", payload)),
    updateChild: (childId, payload) => request(`/api/children/${childId}`, json("PATCH", payload)),
    addChildPhoto: (childId, payload) => request(`/api/children/${childId}/photos`, json("POST", payload)),
    updateChildPhoto: (childId, photoId, payload) => request(`/api/children/${childId}/photos/${photoId}`, json("PATCH", payload)),
    listParentIntakes: (params = {}) => request(`/api/parent-intakes${queryString(params)}`),
    createParentIntake: (payload) => request("/api/parent-intakes", json("POST", payload)),
    createParentIntakeLink: (payload) => request("/api/parent-intake-links", json("POST", payload)),
    acceptParentIntake: (intakeId) => request(`/api/parent-intakes/${intakeId}/accept`, json("POST", {})),
    listCharacterProfiles: (childId) => request(`/api/children/${childId}/character-profiles`),
    createCharacterProfile: (childId, payload) => request(`/api/children/${childId}/character-profiles`, json("POST", payload)),
    getCharacterProfile: (profileId) => request(`/api/character-profiles/${profileId}`),
    updateCharacterProfile: (profileId, payload) => request(`/api/character-profiles/${profileId}`, json("PATCH", payload)),
    createParentCharacterProfile: (parentId, payload) => request(`/api/parents/${parentId}/character-profiles`, json("POST", payload)),
    listPropProfiles: (storybookId) => request(`/api/storybooks/${storybookId}/props`),
    createPropProfile: (storybookId, payload) => request(`/api/storybooks/${storybookId}/props`, json("POST", payload)),
    updatePropProfile: (propId, payload) => request(`/api/prop-profiles/${propId}`, json("PATCH", payload)),
    putPageVisualSubjects: (pageId, payload) => request(`/api/storybook-pages/${pageId}/visual-subjects`, json("PUT", payload)),
    generateReferenceImage: (payload) => request("/api/reference-images/generate", json("POST", payload)),
    getReferenceImage: (referenceImageId) => request(`/api/reference-images/${referenceImageId}`),
    activateReferenceImage: (referenceImageId) => request(`/api/reference-images/${referenceImageId}/activate`, json("POST", {})),
    listStorybookRoles: (storybookId) => request(`/api/storybooks/${storybookId}/roles`),
    updateStorybookRole: (storybookId, roleKey, payload) =>
      request(`/api/storybooks/${storybookId}/roles/${encodeURIComponent(roleKey)}`, json("PATCH", payload)),
    replaceStorybookRoles: (storybookId, payload) => request(`/api/storybooks/${storybookId}/replace-roles`, json("POST", payload)),
  };

  function queryString(params = {}) {
    const query = new URLSearchParams();
    Object.entries(params).forEach(([key, value]) => {
      if (value !== undefined && value !== null && value !== "") {
        query.set(key, value);
      }
    });
    const serialized = query.toString();
    return serialized ? `?${serialized}` : "";
  }

  function currentToken() {
    return window.localStorage.getItem(tokenStorageKey);
  }

  function setToken(token) {
    if (token) {
      window.localStorage.setItem(tokenStorageKey, token);
    }
  }

  function clearToken() {
    window.localStorage.removeItem(tokenStorageKey);
  }
})();
