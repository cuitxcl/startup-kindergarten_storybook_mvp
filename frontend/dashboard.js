const dashboardToast = document.querySelector("#toast");
const navLinks = Array.from(document.querySelectorAll("[data-page-link]"));
const pages = Array.from(document.querySelectorAll("[data-page]"));
const filterButtons = Array.from(document.querySelectorAll("[data-filter]"));
const studioPageTitle = document.querySelector("#studio-page-title");
const studioPreviewTitle = document.querySelector("#studio-preview-title");
const studioPreviewCopy = document.querySelector("#studio-preview-copy");
const studioTitleInput = document.querySelector("#studio-title-input");
const studioCopyInput = document.querySelector("#studio-copy-input");
const choiceButtons = Array.from(document.querySelectorAll(".choice-grid button"));
const workList = document.querySelector("#work-list");
const assetTable = document.querySelector("#asset-table");
const childrenTable = document.querySelector("#children-table");
const pageStripList = document.querySelector("#page-strip-list");
const roleList = document.querySelector("#role-list");

let dashboardState = {
  contentItems: [],
  children: [],
  cases: [],
  selectedStorybook: null,
  pages: [],
  selectedPage: null,
  roles: [],
  latestImageTask: null,
  workItems: [],
  adminContext: {
    classrooms: [],
    teachers: [],
    templates: [],
    parentIntakes: [],
  },
};

function showDashboardToast(message) {
  dashboardToast.textContent = message;
  dashboardToast.classList.remove("is-hidden");
  clearTimeout(showDashboardToast.timer);
  showDashboardToast.timer = setTimeout(() => {
    dashboardToast.classList.add("is-hidden");
  }, 2600);
}

function text(value, fallback = "-") {
  return value === null || value === undefined || value === "" ? fallback : String(value);
}

function escapeHtml(value) {
  return text(value, "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function firstChar(value) {
  return text(value, "孩").slice(0, 1);
}

function contentTypeLabel(value) {
  return {
    plain_storybook: "普通绘本母本",
    custom_storybook: "定制绘本",
  }[value] || text(value);
}

function statusLabel(value) {
  return {
    draft: "草稿",
    generating: "生成中",
    ready: "可交付",
    exporting: "导出中",
    archived: "已归档",
    missing_required: "待补档",
    usable: "可生成",
    complete: "完整",
    active: "启用",
  }[value] || text(value);
}

function selectedStorybookChild() {
  const selected = dashboardState.selectedStorybook;
  if (!selected?.child?.id) {
    return null;
  }
  return dashboardState.children.find((child) => child.id === selected.child.id) || selected.child;
}

function apiMethodCount() {
  return Object.keys(window.KindleleafApi || {}).filter((key) => typeof window.KindleleafApi[key] === "function").length;
}

function showPage(pageId, updateHash = true) {
  const nextPage = pages.find((page) => page.dataset.page === pageId) || pages[0];

  pages.forEach((page) => {
    page.classList.toggle("active", page === nextPage);
  });

  navLinks.forEach((link) => {
    link.classList.toggle("active", link.dataset.pageLink === nextPage.dataset.page);
  });

  if (updateHash) {
    history.replaceState(null, "", `#${nextPage.id}`);
  }
}

function bindToastButtons(scope = document) {
  Array.from(scope.querySelectorAll("[data-toast]")).forEach((button) => {
    if (button.dataset.toastBound === "true") {
      return;
    }
    button.dataset.toastBound = "true";
    button.addEventListener("click", () => {
      showDashboardToast(button.dataset.toast);
    });
  });
}

function setText(selector, value) {
  const node = document.querySelector(selector);
  if (node) {
    node.textContent = value;
  }
}

function setLatestImageTask(detail) {
  dashboardState.latestImageTask = detail;
  setText(
    "#latest-image-task-status",
    detail ? `${detail.task_type} · ${detail.status} · ${detail.outputs?.length || 0} 张候选图` : "暂无图片任务"
  );
}

function rowButton(label, toast) {
  const button = document.createElement("button");
  button.className = "secondary compact-button";
  button.type = "button";
  button.dataset.toast = toast;
  button.textContent = label;
  return button;
}

function actionButton(label, action, dataset = {}) {
  const button = document.createElement("button");
  button.className = "secondary compact-button";
  button.type = "button";
  button.dataset.action = action;
  Object.entries(dataset).forEach(([key, value]) => {
    if (value !== undefined && value !== null) {
      button.dataset[key] = String(value);
    }
  });
  button.textContent = label;
  return button;
}

function renderDashboard(summary) {
  const counts = summary.work_counts || {};
  const classroom = summary.current_classroom;
  setText("#teacher-name", `${text(summary.teacher?.name, "老师")} 控制台`);
  setText("#current-classroom", classroom ? `${classroom.name} · ${classroom.child_count} 位孩子` : "未选择班级");
  setText("#today-subtitle", `${text(summary.current_school?.name, "当前园所")} · ${text(summary.teacher?.role, "teacher")}`);
  setText("#metric-ready-export", counts.ready_to_export ?? 0);
  setText("#metric-running-tasks", (counts.story_generating || 0) + (counts.running_image_tasks || 0));
  setText("#metric-missing-profile", counts.children_missing_profile ?? 0);

  const ready = counts.ready_to_export || 0;
  const missing = counts.children_missing_profile || 0;
  setText("#today-alert-title", ready > 0 ? `${ready} 本绘本还没有完成导出确认` : "当前没有待导出的绘本");
  setText(
    "#today-alert-copy",
    missing > 0 ? `${missing} 份孩子档案需要补齐，生成前建议先处理。` : "孩子档案状态正常，可以继续生成和交付。"
  );
}

function buildWorkItems() {
  const exportItems = dashboardState.contentItems
    .filter((item) => item.export_status === "not_exported" || item.status === "ready")
    .slice(0, 4)
    .map((item) => ({
      storybookId: item.storybook_id,
      kind: "export",
      badge: "待导出",
      title: item.child ? `${item.child.name}的《${item.title}》` : `《${item.title}》`,
      copy: `${contentTypeLabel(item.content_type)} · ${statusLabel(item.status)} · ${text(item.share_scope, "private")}`,
      meta: "交付确认",
      action: "确认导出",
    }));

  const profileItems = dashboardState.children
    .filter((child) => child.profile_completion_status !== "complete")
    .slice(0, 4)
    .map((child) => ({
      childId: child.id,
      kind: "profile",
      badge: "待补档",
      title: `${child.name}的孩子档案`,
      copy: `${text(child.age, "-")} 岁 · ${text(child.teaching_focus, "待补充教学关注")} · ${statusLabel(child.profile_completion_status)}`,
      meta: "生成前",
      action: "补充",
    }));

  const storyItems = dashboardState.cases.slice(0, 4).map((item) => ({
    caseId: item.id,
    kind: "story",
    badge: "可派生",
    title: `《${item.title}》普通绘本母本`,
    copy: `${text(item.theme)} · ${text(item.teaching_goal)} · ${text(item.target_age_group, "全年龄")}`,
    meta: "母本制作",
    action: "派生",
  }));

  dashboardState.workItems = [...exportItems, ...profileItems, ...storyItems].slice(0, 8);
}

function renderWorkList(filter = "all") {
  const visibleItems = dashboardState.workItems.filter((item) => filter === "all" || item.kind === filter);
  workList.replaceChildren();

  if (visibleItems.length === 0) {
    const empty = document.createElement("article");
    empty.className = "empty-state";
    empty.textContent = "当前筛选下没有待办。";
    workList.append(empty);
    return;
  }

  visibleItems.forEach((item, index) => {
    const row = document.createElement("article");
    row.className = "work-row";
    row.dataset.kind = item.kind;
    row.innerHTML = `
      <div class="work-status ${index === 0 ? "high" : ""}">${index + 1}</div>
      <div class="work-copy">
        <span class="status-pill ${item.kind === "profile" ? "muted" : ""}">${escapeHtml(item.badge)}</span>
        <h3>${escapeHtml(item.title)}</h3>
        <p>${escapeHtml(item.copy)}</p>
      </div>
      <div class="work-meta"><strong>${escapeHtml(item.meta)}</strong><span>${item.kind === "export" ? "离园沟通" : "内容生产"}</span></div>
      <div class="work-actions"></div>
    `;
    const actions = row.querySelector(".work-actions");
    if (item.kind === "export") {
      actions.append(actionButton(item.action, "export-storybook", { storybookId: item.storybookId }));
    } else if (item.kind === "profile") {
      actions.append(actionButton(item.action, "edit-child", { childId: item.childId }));
    } else if (item.kind === "story") {
      actions.append(actionButton(item.action, "generate-storybook", { caseId: item.caseId }));
    } else {
      actions.append(rowButton(item.action, `已进入${item.badge}处理：${item.title}`));
    }
    actions.append(rowButton("预览", `已打开预览：${item.title}`));
    workList.append(row);
  });
  bindToastButtons(workList);
}

function renderAssets() {
  const rows = [
    ...dashboardState.cases.map((item) => ({
      title: item.title,
      type: "普通绘本母本",
      status: "可派生",
      reuse: text(item.theme),
    })),
    ...dashboardState.contentItems.map((item) => ({
      storybookId: item.storybook_id,
      title: item.child ? `${item.child.name}的${item.title}` : item.title,
      type: contentTypeLabel(item.content_type),
      status: statusLabel(item.status),
      reuse: `${item.page_count || 0} 页`,
    })),
  ].slice(0, 12);

  const head = assetTable.querySelector(".asset-head");
  assetTable.replaceChildren(head);

  rows.forEach((item) => {
    const row = document.createElement("div");
    row.setAttribute("role", "row");
    row.innerHTML = `<strong>${escapeHtml(item.title)}</strong><span>${escapeHtml(item.type)}</span><span>${escapeHtml(item.status)}</span><span>${escapeHtml(item.reuse)}</span>`;
    if (item.storybookId) {
      row.append(actionButton("打开", "open-storybook", { storybookId: item.storybookId }));
    } else {
      row.append(rowButton("打开", `已打开《${item.title}》。`));
    }
    assetTable.append(row);
  });
  bindToastButtons(assetTable);
  setText("#metric-case-count", dashboardState.cases.length);
}

function renderChildren() {
  const head = childrenTable.querySelector(".asset-head");
  childrenTable.replaceChildren(head);

  dashboardState.children.forEach((child) => {
    const interests = Array.isArray(child.interest_tags) && child.interest_tags.length > 0 ? child.interest_tags.join("、") : "-";
    const row = document.createElement("div");
    row.setAttribute("role", "row");
    row.innerHTML = `
      <strong><span class="avatar">${escapeHtml(firstChar(child.nickname || child.name))}</span>${escapeHtml(text(child.nickname || child.name))}</strong>
      <span>${escapeHtml(text(child.age))} 岁</span>
      <span>${escapeHtml(text(child.teaching_focus, "未填写"))}</span>
      <span>${escapeHtml(interests)}</span>
      <span>${escapeHtml(statusLabel(child.profile_completion_status))}</span>
    `;
    row.append(actionButton(child.profile_completion_status === "complete" ? "编辑" : "补充", "edit-child", { childId: child.id }));
    childrenTable.append(row);
  });
  bindToastButtons(childrenTable);
  setText("#metric-child-count", dashboardState.children.length);
}

function renderRoles(rolesResponse) {
  const roles = rolesResponse.items || [];
  dashboardState.roles = roles;
  if (!roleList) {
    return;
  }
  roleList.replaceChildren();
  if (roles.length === 0) {
    roleList.append(Object.assign(document.createElement("article"), { className: "empty-state", textContent: "当前绘本还没有角色。" }));
    return;
  }
  roles.forEach((role) => {
    const item = document.createElement("article");
    item.innerHTML = `
      <span class="avatar">${escapeHtml(firstChar(role.display_name || role.role_key))}</span>
      <div><strong>${escapeHtml(role.role_key)} · ${escapeHtml(role.display_name)}</strong><p>${escapeHtml(role.role_type)} · ${role.character_profile_id ? "已绑定角色画像" : "未绑定画像"}</p></div>
    `;
    if (role.role_type === "child" || role.child_id) {
      item.append(actionButton(role.character_profile_id ? "重建参考图" : "生成参考图", "generate-reference-image", { roleKey: role.role_key }));
    }
    roleList.append(item);
  });
}

function selectStoryPage(thumb) {
  Array.from(document.querySelectorAll(".page-thumb")).forEach((item) => item.classList.toggle("active", item === thumb));

  if (!studioPageTitle || !studioPreviewTitle || !studioPreviewCopy || !studioTitleInput || !studioCopyInput) {
    return;
  }

  studioPageTitle.textContent = thumb.dataset.pageTitle;
  studioPreviewTitle.textContent = thumb.dataset.pageCopy;
  studioPreviewCopy.textContent = thumb.dataset.pageNote;
  studioTitleInput.value = thumb.dataset.pageCopy;
  studioCopyInput.value = thumb.dataset.pageNote;
  dashboardState.selectedPage = dashboardState.pages.find((page) => page.id === thumb.dataset.pageId) || null;
  const lockButton = document.querySelector('[data-action="toggle-lock-page"]');
  if (lockButton && dashboardState.selectedPage) {
    lockButton.textContent = dashboardState.selectedPage.is_locked ? "解锁页面" : "锁定页面";
  }
}

function renderPages(pagesResponse) {
  const storyPages = pagesResponse.items || [];
  dashboardState.pages = storyPages;
  dashboardState.selectedPage = storyPages[0] || null;
  pageStripList.replaceChildren();
  setText("#page-count-badge", `${storyPages.length} 页`);

  if (storyPages.length === 0) {
    const empty = document.createElement("article");
    empty.className = "empty-state";
    empty.textContent = "当前绘本还没有页面。";
    pageStripList.append(empty);
    return;
  }

  storyPages.forEach((page, index) => {
    const button = document.createElement("button");
    button.className = `page-thumb ${index === 0 ? "active" : ""} ${page.is_locked ? "locked" : ""}`;
    button.type = "button";
    button.dataset.pageTitle = page.page_role === "cover" ? "封面" : `第 ${page.page_number} 页`;
    button.dataset.pageCopy = page.page_title || page.body_text;
    button.dataset.pageNote = page.body_text;
    button.dataset.pageId = page.id;
    button.innerHTML = `<span>${page.page_role === "cover" ? "Cover" : `Page ${escapeHtml(page.page_number)}`}</span><strong>${escapeHtml(page.page_title || page.body_text)}</strong>`;
    button.addEventListener("click", () => selectStoryPage(button));
    pageStripList.append(button);
  });

  selectStoryPage(pageStripList.querySelector(".page-thumb"));
}

async function loadStudioPages() {
  const selected =
    dashboardState.contentItems.find((item) => item.storybook_id === dashboardState.selectedStorybook?.storybook_id) ||
    dashboardState.contentItems[0];
  dashboardState.selectedStorybook = selected;
  if (!selected) {
    renderPages({ items: [] });
    return;
  }

  setText("#storybook-roles-label", selected.child ? `${selected.child.name} · ${selected.theme}` : selected.theme);
  const [pagesResponse, rolesResponse] = await Promise.all([
    window.KindleleafApi.listPages(selected.storybook_id),
    window.KindleleafApi.listStorybookRoles(selected.storybook_id),
  ]);
  renderPages(pagesResponse);
  renderRoles(rolesResponse);
}

async function refreshDashboard() {
  await loadDashboardData({ silent: true });
}

async function loadDashboardData(options = {}) {
  try {
    const [dashboard, contentItems, children, cases] = await Promise.all([
      window.KindleleafApi.getDashboard(),
      window.KindleleafApi.listContentItems(),
      window.KindleleafApi.listChildren(),
      window.KindleleafApi.listCases(),
    ]);

    dashboardState.contentItems = contentItems.items || [];
    dashboardState.children = children.items || [];
    dashboardState.cases = cases.items || [];

    renderDashboard(dashboard);
    buildWorkItems();
    renderWorkList(document.querySelector("[data-filter].active")?.dataset.filter || "all");
    renderAssets();
    renderChildren();
    setText("#metric-admin-api-count", apiMethodCount());
    await loadStudioPages();
    if (!options.silent) {
      showDashboardToast("已从后端接口同步控制台数据。");
    }
  } catch (error) {
    showDashboardToast(`后端接口连接失败：${error.message}`);
    const message = "无法连接后端接口，请确认 Loco 服务已启动，或使用 ?api=http://127.0.0.1:5150 指定地址。";
    if (workList) {
      workList.replaceChildren(Object.assign(document.createElement("article"), { className: "empty-state", textContent: message }));
    }
  }
}

async function syncAdminContext(options = {}) {
  const [school, teacher, classrooms, teachers, templates, parentIntakes] = await Promise.allSettled([
    window.KindleleafApi.getCurrentSchool(),
    window.KindleleafApi.getCurrentTeacher(),
    window.KindleleafApi.listClassrooms(),
    window.KindleleafApi.listTeachers(),
    window.KindleleafApi.listTemplates(),
    window.KindleleafApi.listParentIntakes(),
  ]);
  if (school.status === "fulfilled") {
    setText("#today-subtitle", `${school.value.name} · ${school.value.status}`);
  }
  if (teacher.status === "fulfilled") {
    setText("#teacher-name", `${teacher.value.name} 控制台`);
  }
  dashboardState.adminContext.classrooms = classrooms.status === "fulfilled" ? classrooms.value.items || [] : [];
  dashboardState.adminContext.teachers = teachers.status === "fulfilled" ? teachers.value.items || [] : [];
  dashboardState.adminContext.templates = templates.status === "fulfilled" ? templates.value.items || [] : [];
  dashboardState.adminContext.parentIntakes = parentIntakes.status === "fulfilled" ? parentIntakes.value.items || [] : [];
  setText("#metric-classroom-count", dashboardState.adminContext.classrooms.length);
  setText("#metric-template-count", dashboardState.adminContext.templates.length);
  setText("#metric-intake-count", dashboardState.adminContext.parentIntakes.length);
  setText("#metric-admin-api-count", apiMethodCount());
  const blocked = [school, teacher, classrooms, teachers, templates, parentIntakes].filter((item) => item.status === "rejected").length;
  if (!options.silent) {
    showDashboardToast(blocked ? `系统上下文已同步，${blocked} 组接口受权限限制。` : "系统上下文已同步。");
  }
}

function requireSelectedStorybook() {
  if (!dashboardState.selectedStorybook) {
    throw new Error("请先选择一本绘本。");
  }
  return dashboardState.selectedStorybook;
}

function requireSelectedPage() {
  if (!dashboardState.selectedPage) {
    throw new Error("请先选择一个页面。");
  }
  return dashboardState.selectedPage;
}

async function saveCurrentPage() {
  const storybook = requireSelectedStorybook();
  const page = requireSelectedPage();
  const updated = await window.KindleleafApi.updatePage(storybook.storybook_id, page.id, {
    page_title: studioTitleInput.value,
    body_text: studioCopyInput.value,
  });
  dashboardState.selectedPage = updated;
  showDashboardToast("当前页已保存到后端。");
  await loadStudioPages();
}

async function toggleCurrentPageLock() {
  const storybook = requireSelectedStorybook();
  const page = requireSelectedPage();
  const updated = await window.KindleleafApi.updatePage(storybook.storybook_id, page.id, {
    is_locked: !page.is_locked,
  });
  dashboardState.selectedPage = updated;
  showDashboardToast(updated.is_locked ? "当前页已锁定。" : "当前页已解锁。");
  await loadStudioPages();
}

async function rewriteCurrentPage() {
  const storybook = requireSelectedStorybook();
  const page = requireSelectedPage();
  const rewritten = await window.KindleleafApi.rewritePage(storybook.storybook_id, page.id, {
    override_locked: page.is_locked,
  });
  dashboardState.selectedPage = rewritten;
  showDashboardToast("当前页已由后端重写。");
  await loadStudioPages();
}

async function redrawCurrentPage() {
  const page = requireSelectedPage();
  const detail = await window.KindleleafApi.createPageImageTask(page.id, {
    style_id: activeStyleId(),
    prompt_template_version: "v1",
    reference_image_ids: [],
    regeneration_reason: "teacher_requested_redraw",
  });
  setLatestImageTask(detail);
  showDashboardToast(`单页重绘任务已提交：${detail.status}`);
  await refreshDashboard();
}

function requireLatestImageTask() {
  if (!dashboardState.latestImageTask) {
    throw new Error("请先提交一次图片任务。");
  }
  return dashboardState.latestImageTask;
}

async function selectLatestImageOutput() {
  const task = requireLatestImageTask();
  const output = (task.outputs || []).find((item) => item.review_status !== "rejected") || task.outputs?.[0];
  if (!output) {
    throw new Error("当前图片任务没有候选图。");
  }
  await window.KindleleafApi.selectImageOutput(output.id);
  await window.KindleleafApi.reviewImageOutput(output.id, {
    review_action: "approve",
    notes: "teacher_selected_from_dashboard",
  });
  const refreshed = await window.KindleleafApi.getImageTask(task.id);
  setLatestImageTask(refreshed);
  showDashboardToast("候选图已选中并审核通过。");
  await refreshDashboard();
}

async function viewLatestReviewEvents() {
  const task = requireLatestImageTask();
  const events = await window.KindleleafApi.listReviewEvents(task.id);
  showDashboardToast(`当前图片任务有 ${events.total || 0} 条审核记录。`);
}

async function viewGenerationCosts() {
  const storybook = dashboardState.selectedStorybook;
  const costs = await window.KindleleafApi.listGenerationCosts(storybook?.storybook_id);
  const total = (costs.items || []).reduce((sum, item) => sum + Number(item.total_cost || 0), 0);
  showDashboardToast(`生成成本记录 ${costs.total || 0} 条，合计 ${total.toFixed(2)}。`);
}

async function registerUploadedAsset() {
  const filename = window.prompt("文件名", "child-reference.jpg");
  if (!filename) {
    return;
  }
  const intent = await window.KindleleafApi.createUploadIntent({
    asset_type: "child_photo",
    filename,
    mime_type: "image/jpeg",
    file_size: 1024,
  });
  const asset = await window.KindleleafApi.createAsset({
    asset_type: intent.asset_type,
    storage_url: intent.upload_url,
    storage_key: intent.storage_key,
    mime_type: intent.mime_type,
    file_size: intent.file_size,
    metadata_json: {
      upload_intent_id: intent.id,
      source: "dashboard_upload_registration",
    },
  });
  showDashboardToast(`资产已登记：${asset.id}`);
}

function profilePayloadFromChild(child) {
  const name = text(child.nickname || child.name, "孩子");
  const hair = child.hair || window.prompt("角色发型", "黑色短发");
  const outfit = child.usual_outfit || window.prompt("常穿服装", "黄色卫衣");
  if (!hair || !outfit) {
    return null;
  }
  const interests = Array.isArray(child.interest_tags) ? child.interest_tags.filter(Boolean) : [];
  return {
    name: child.name || name,
    nickname: child.nickname || name,
    age_group: child.age_group || normalizeAgeGroup(child.age) || "5-6",
    gender_expression: child.gender_expression,
    hair,
    body_proportion: "幼儿比例",
    outfit_top: outfit,
    signature_colors: child.favorite_color ? [child.favorite_color] : [],
    interest_elements: interests,
    visual_must_keep: [hair, outfit, `${name}的圆脸辨识度`],
    negative_rules: ["不要成人化比例", "不要改变主要发型"],
  };
}

async function ensureCharacterProfileForRole(role, child) {
  if (role.character_profile_id) {
    return role.character_profile_id;
  }
  const profiles = await window.KindleleafApi.listCharacterProfiles(child.id);
  const existing = (profiles.items || []).find((item) => item.status !== "superseded");
  if (existing) {
    return existing.id;
  }
  const payload = profilePayloadFromChild(child);
  if (!payload) {
    throw new Error("已取消角色画像创建。");
  }
  const profile = await window.KindleleafApi.createCharacterProfile(child.id, payload);
  return profile.id;
}

async function generateReferenceImageForRole(roleKey) {
  const storybook = requireSelectedStorybook();
  const role = dashboardState.roles.find((item) => item.role_key === roleKey) || dashboardState.roles[0];
  if (!role) {
    throw new Error("当前绘本没有可绑定的角色。");
  }
  const child = role.child_id
    ? dashboardState.children.find((item) => item.id === role.child_id)
    : selectedStorybookChild();
  if (!child?.id) {
    throw new Error("当前角色没有关联孩子档案，无法生成儿童参考图。");
  }
  const characterProfileId = await ensureCharacterProfileForRole(role, child);
  const reference = await window.KindleleafApi.generateReferenceImage({
    subject_type: "child_character",
    character_profile_id: characterProfileId,
    style_id: activeStyleId(),
  });
  const activeReference = await window.KindleleafApi.activateReferenceImage(reference.id);
  await window.KindleleafApi.updateStorybookRole(storybook.storybook_id, role.role_key, {
    character_profile_id: characterProfileId,
    child_id: child.id,
  });
  showDashboardToast(`参考图已启用：${activeReference.style_id}`);
  await loadStudioPages();
}

async function deleteCurrentPage() {
  const storybook = requireSelectedStorybook();
  const page = requireSelectedPage();
  if (!window.confirm("确认删除当前页？")) {
    return;
  }
  const pagesResponse = await window.KindleleafApi.deletePage(storybook.storybook_id, page.id);
  showDashboardToast("当前页已删除。");
  renderPages(pagesResponse);
  await refreshDashboard();
}

async function addPageToCurrentStorybook() {
  const storybook = requireSelectedStorybook();
  const nextNumber = dashboardState.pages.length + 1;
  await window.KindleleafApi.addPage(storybook.storybook_id, {
    page_number: nextNumber,
    page_role: "story",
    page_title: `第 ${nextNumber} 页`,
    body_text: "请在这里填写新的故事正文。",
    scene_spec_status: "draft",
  });
  showDashboardToast("新页面已创建。");
  await loadStudioPages();
  await refreshDashboard();
}

async function exportStorybook(storybookId) {
  const storybook = storybookId
    ? dashboardState.contentItems.find((item) => item.storybook_id === storybookId)
    : requireSelectedStorybook();
  if (!storybook) {
    throw new Error("没有找到要导出的绘本。");
  }
  await window.KindleleafApi.createExport(storybook.storybook_id, {
    export_type: "pdf",
    include_teacher_tips: true,
    page_size: "A4",
    quality: "print",
    allow_text_only: true,
  });
  showDashboardToast("已创建后端导出任务。");
  await refreshDashboard();
}

async function shareStorybook() {
  const storybook = requireSelectedStorybook();
  const share = await window.KindleleafApi.createShareLink(storybook.storybook_id, {
    share_scope: "family",
    anonymize_child_name: true,
    anonymize_parent_info: true,
    create_qrcode: true,
  });
  showDashboardToast(`分享链接已创建：${share.url}`);
  await refreshDashboard();
}

async function checkShareScope() {
  const storybook = requireSelectedStorybook();
  const [shares, library] = await Promise.all([
    window.KindleleafApi.listShareLinks(storybook.storybook_id),
    window.KindleleafApi.listSharedLibrary({ content_type: storybook.content_type || "" }),
  ]);
  showDashboardToast(`当前绘本分享链接 ${shares.total || 0} 个，母本库可见 ${library.total || 0} 本。`);
}

async function viewSharedLibrary() {
  const library = await window.KindleleafApi.listSharedLibrary({ share_scope: "school", content_type: "plain_storybook" });
  const first = (library.items || [])[0];
  if (!first) {
    showDashboardToast("当前没有可复用的园内母本。");
    return;
  }
  if (!window.confirm(`复制《${first.title}》到当前园所工作区？`)) {
    showDashboardToast(`母本库可见 ${library.total || 0} 本。`);
    return;
  }
  const child = selectedStorybookChild() || dashboardState.children[0];
  const clone = await window.KindleleafApi.cloneSharedStorybook(first.storybook_id, {
    target_child_id: child?.id,
    title_override: child ? `${child.nickname || child.name}的${first.title}` : `${first.title} 改编`,
    replace_sensitive_roles: true,
    regenerate_images: Boolean(child),
  });
  showDashboardToast(`已复制母本：${clone.title}`);
  await refreshDashboard();
  await openStorybook(clone.id);
}

async function submitPlatformReview() {
  const storybook = requireSelectedStorybook();
  const result = await window.KindleleafApi.submitPlatformReview(storybook.storybook_id);
  showDashboardToast(`平台审核已提交：${result.review_status}`);
  await refreshDashboard();
}

async function manageOrganization() {
  const [school, currentTeacher, classrooms] = await Promise.all([
    window.KindleleafApi.getCurrentSchool(),
    window.KindleleafApi.getCurrentTeacher(),
    window.KindleleafApi.listClassrooms(),
  ]);
  const name = window.prompt("测试班级名称", `接口验证班-${Date.now().toString().slice(-4)}`);
  if (!name) {
    showDashboardToast(`当前园所：${school.name}，默认老师：${currentTeacher.name}。`);
    return;
  }
  const created = await window.KindleleafApi.createClassroom({
    name,
    teacher_id: currentTeacher.id,
    grade_level: "混龄",
  });
  await window.KindleleafApi.updateClassroom(created.id, {
    status: "archived",
  });
  await window.KindleleafApi.updateCurrentSchool({
    name: school.name,
  });
  await syncAdminContext({ silent: true });
  showDashboardToast(`组织接口已验证：${classrooms.total || 0} 个原班级，新增后已归档。`);
}

async function manageTemplates() {
  const sourceCase = dashboardState.cases[0];
  if (!sourceCase) {
    throw new Error("当前没有可读取的案例。");
  }
  const detail = await window.KindleleafApi.getCase(sourceCase.id);
  await window.KindleleafApi.cloneCase(sourceCase.id, {
    mode: "plain_storybook",
    title_override: `${sourceCase.title} 接口副本`,
  });
  let templateStatus = "模板接口已验证";
  try {
    const templates = await window.KindleleafApi.listTemplates();
    let template = (templates.items || [])[0];
    if (!template) {
      template = await window.KindleleafApi.createTemplate({
        title: `${sourceCase.title} 模板`,
        content_type: "plain_storybook",
        theme: sourceCase.theme,
        teaching_goal: sourceCase.teaching_goal,
        target_age_group: sourceCase.target_age_group,
        page_count: 2,
        template_outline_json: {
          pages: [{ page_role: "cover" }, { page_role: "closing" }],
        },
        default_role_manifest_json: {
          protagonist: { role_type: "default_character", display_name: "小朋友" },
        },
        status: "draft",
      });
    }
    await window.KindleleafApi.getTemplate(template.id);
    await window.KindleleafApi.updateTemplate(template.id, {
      teaching_goal: template.teaching_goal || sourceCase.teaching_goal,
    });
  } catch (error) {
    templateStatus = `模板接口受权限限制：${error.message}`;
  }
  await syncAdminContext({ silent: true });
  showDashboardToast(`内容接口已验证：案例 ${detail.pages?.length || 0} 页，${templateStatus}。`);
}

async function manageParentIntake() {
  const child = selectedStorybookChild() || dashboardState.children[0];
  const link = await window.KindleleafApi.createParentIntakeLink({
    child_id: child?.id,
  });
  const intake = await window.KindleleafApi.createParentIntake({
    invite_token: link.invite_token,
    parent: {
      name: "接口验证家长",
      relationship_to_child: "妈妈",
      phone: "13800000001",
    },
    child: {
      name: child?.name || "接口验证孩子",
      nickname: child?.nickname || child?.name || "验证孩子",
      age: child?.age || 5,
      age_group: child?.age_group || "5-6",
      hair: child?.hair || "黑色短发",
      usual_outfit: child?.usual_outfit || "黄色卫衣",
      interest_tags: child?.interest_tags || ["积木"],
    },
    parent_character_profile: {
      role: "mother",
      hair: "黑色长发",
      outfit_top: "绿色外套",
      visual_must_keep: ["黑色长发", "绿色外套", "温和表情"],
    },
    photo_asset_ids: [],
  });
  await window.KindleleafApi.listParentIntakes();
  const accepted = await window.KindleleafApi.acceptParentIntake(intake.id);
  await refreshDashboard();
  await syncAdminContext({ silent: true });
  showDashboardToast(`家长采集已接收为孩子档案：${accepted.child_id}`);
}

async function createChildPhotoAsset() {
  const intent = await window.KindleleafApi.createUploadIntent({
    asset_type: "child_photo",
    filename: "profile-photo.jpg",
    mime_type: "image/jpeg",
    file_size: 2048,
  });
  return window.KindleleafApi.createAsset({
    asset_type: intent.asset_type,
    storage_url: intent.upload_url,
    storage_key: intent.storage_key,
    mime_type: intent.mime_type,
    file_size: intent.file_size,
    metadata_json: {
      upload_intent_id: intent.id,
      source: "child_photo_management",
    },
  });
}

async function manageChildPhoto() {
  const child = selectedStorybookChild() || dashboardState.children[0];
  if (!child?.id) {
    throw new Error("请先创建孩子档案。");
  }
  const asset = await createChildPhotoAsset();
  await window.KindleleafApi.getAsset(asset.id);
  const photo = await window.KindleleafApi.addChildPhoto(child.id, {
    image_asset_id: asset.id,
    photo_type: "portrait",
    is_primary: true,
    consent_status: "granted",
  });
  await window.KindleleafApi.updateChildPhoto(child.id, photo.id, {
    is_primary: true,
    consent_status: "granted",
  });
  const detail = await window.KindleleafApi.getChild(child.id);
  await refreshDashboard();
  showDashboardToast(`照片接口已验证：${detail.photos?.length || 0} 张照片。`);
}

async function manageVisualSubjects() {
  const storybook = requireSelectedStorybook();
  const page = requireSelectedPage();
  const prop = await window.KindleleafApi.createPropProfile(storybook.storybook_id, {
    child_id: selectedStorybookChild()?.id,
    name: "黄色小书包",
    shape: "圆角书包",
    primary_color: "黄色",
    material_style: "布面",
    size_description: "适合幼儿背的小号书包",
    visual_must_keep: ["黄色", "圆角书包", "小号"],
  });
  const activeRole = dashboardState.roles[0];
  const subjects = [];
  if (activeRole?.id) {
    subjects.push({
      subject_type: "storybook_role",
      storybook_role_id: activeRole.id,
      importance: "primary",
      placement_hint: "画面中央",
    });
  }
  subjects.push({
    subject_type: "prop",
    prop_profile_id: prop.id,
    importance: "secondary",
    placement_hint: "角色旁边",
  });
  await window.KindleleafApi.putPageVisualSubjects(page.id, { subjects });
  if (activeRole) {
    await window.KindleleafApi.replaceStorybookRoles(storybook.storybook_id, {
      replacements: [{
        role_key: activeRole.role_key,
        role_type: activeRole.role_type,
        child_id: activeRole.child_id,
        character_profile_id: activeRole.character_profile_id,
        parent_character_profile_id: activeRole.parent_character_profile_id,
        prop_profile_id: activeRole.prop_profile_id,
      }],
    });
  }
  await window.KindleleafApi.updatePropProfile(prop.id, {
    status: "active",
  });
  await loadStudioPages();
  showDashboardToast("视觉主体、道具画像和角色替换接口已验证。");
}

async function manageDeliveryDetails() {
  const storybook = requireSelectedStorybook();
  const exports = await window.KindleleafApi.listExports(storybook.storybook_id);
  let exportItem = (exports.items || [])[0];
  if (!exportItem) {
    exportItem = await window.KindleleafApi.createExport(storybook.storybook_id, {
      export_type: "pdf",
      include_teacher_tips: true,
      page_size: "A4",
      quality: "preview",
      allow_text_only: true,
    });
  }
  await window.KindleleafApi.getExport(exportItem.id);
  const activity = await window.KindleleafApi.listContentItemActivity(storybook.storybook_id);
  const shares = await window.KindleleafApi.listShareLinks(storybook.storybook_id);
  const share = (shares.items || [])[0] || await window.KindleleafApi.createShareLink(storybook.storybook_id, {
    share_scope: "family",
    anonymize_child_name: true,
    anonymize_parent_info: true,
  });
  await window.KindleleafApi.updateShareLink(share.id, {
    anonymize_child_name: true,
    anonymize_parent_info: true,
  });
  showDashboardToast(`交付明细已验证：活动 ${activity.total || 0} 条，分享 ${shares.total || 0} 个。`);
}

async function manageStorybookVariants() {
  const storybook = requireSelectedStorybook();
  await window.KindleleafApi.getStorybook(storybook.storybook_id);
  const duplicate = await window.KindleleafApi.duplicateStorybook(storybook.storybook_id, {
    title_override: `${storybook.title} 复制版`,
  });
  const child = selectedStorybookChild() || dashboardState.children[0];
  const plainSource = dashboardState.contentItems.find((item) => item.content_type === "plain_storybook") || duplicate;
  if (child?.id && plainSource?.content_type === "plain_storybook") {
    await window.KindleleafApi.deriveCustomStorybook(plainSource.storybook_id || plainSource.id, {
      child_id: child.id,
      title_override: `${child.nickname || child.name}的${plainSource.title}`,
    });
  }
  await refreshDashboard();
  showDashboardToast(`读本明细、复制和派生接口已验证：${duplicate.title}`);
}

async function manageImageDetails() {
  const task = dashboardState.latestImageTask;
  if (task?.outputs?.[0]?.image_asset?.id) {
    await window.KindleleafApi.getAsset(task.outputs[0].image_asset.id);
  }
  if (task?.status === "failed") {
    const retried = await window.KindleleafApi.retryImageTask(task.id, {
      retry_reason: "provider_timeout",
      override_scene_spec_json: task.scene_spec_json || {},
    });
    setLatestImageTask(retried);
    showDashboardToast(`图片任务已重试：${retried.status}`);
    return;
  }
  const costs = await window.KindleleafApi.listGenerationCosts();
  showDashboardToast(`图片明细接口已验证，成本记录 ${costs.total || 0} 条。`);
}

function showRouteCoverage() {
  const covered = apiMethodCount();
  showDashboardToast(`前端 API client 当前暴露 ${covered} 个方法，系统页提供 8 组管理入口。`);
}

function activeStyleId() {
  const active = document.querySelector(".choice-grid button.active");
  return {
    "柔和彩铅": "soft-colored-pencil",
    "扁平贴纸": "flat-sticker",
    "水彩绘本": "watercolor-storybook",
  }[active?.textContent?.trim()] || "soft-colored-pencil";
}

async function renameFirstRole() {
  const storybook = requireSelectedStorybook();
  const role = dashboardState.roles[0];
  if (!role) {
    throw new Error("当前绘本还没有可编辑角色。");
  }
  const nextName = window.prompt("角色显示名称", role.display_name);
  if (!nextName) {
    return;
  }
  await window.KindleleafApi.updateStorybookRole(storybook.storybook_id, role.role_key, {
    display_name: nextName,
  });
  showDashboardToast("角色名称已更新。");
  await loadStudioPages();
}

async function generateStorybookFromCase(caseId) {
  const sourceCase = dashboardState.cases.find((item) => item.id === caseId) || dashboardState.cases[0];
  const child = dashboardState.children.find((item) => item.profile_completion_status !== "missing_required") || dashboardState.children[0];
  if (!sourceCase) {
    throw new Error("当前没有可派生的母本。");
  }
  if (!child) {
    throw new Error("请先创建孩子档案。");
  }
  const response = await window.KindleleafApi.generateStorybook({
    content_type: "custom_storybook",
    child_id: child.id,
    case_storybook_id: sourceCase.id,
    title_override: `${child.nickname || child.name}的${sourceCase.title}`,
    style_id: "soft-colored-pencil",
    reading_age_group: child.age_group || "5-6",
    teaching_goal: child.teaching_focus || sourceCase.teaching_goal,
    generation_options: {
      source: "dashboard",
    },
  });
  showDashboardToast(`已创建绘本：${response.storybook.title}`);
  await refreshDashboard();
  await openStorybook(response.storybook.id);
}

async function createChildFromPrompt() {
  const name = window.prompt("孩子姓名或昵称");
  if (!name) {
    return;
  }
  const ageValue = window.prompt("年龄，例如 5", "5");
  const focus = window.prompt("最近教学关注", "练习分享和轮流");
  await window.KindleleafApi.createChild({
    name,
    nickname: name,
    age: ageValue ? Number(ageValue) : undefined,
    age_group: normalizeAgeGroup(ageValue ? Number(ageValue) : null),
    interest_tags: [],
    teacher_observation_tags: [],
    teaching_focus: focus || undefined,
  });
  showDashboardToast("孩子档案已创建。");
  await refreshDashboard();
}

function normalizeAgeGroup(age) {
  if (!age) {
    return undefined;
  }
  if (age <= 4) {
    return "3-4";
  }
  if (age === 5) {
    return "5-6";
  }
  if (age >= 6) {
    return "6-7";
  }
  return "4-5";
}

async function editChildFromPrompt(childId) {
  const child = dashboardState.children.find((item) => item.id === childId);
  if (!child) {
    throw new Error("没有找到孩子档案。");
  }
  const focus = window.prompt("更新教学关注", child.teaching_focus || "");
  if (focus === null) {
    return;
  }
  const interests = window.prompt("兴趣标签，用顿号或逗号分隔", (child.interest_tags || []).join("、"));
  if (interests === null) {
    return;
  }
  await window.KindleleafApi.updateChild(child.id, {
    teaching_focus: focus,
    interest_tags: interests.split(/[、,，]/).map((item) => item.trim()).filter(Boolean),
  });
  showDashboardToast("孩子档案已更新。");
  await refreshDashboard();
}

async function openStorybook(storybookId) {
  const selected = dashboardState.contentItems.find((item) => item.storybook_id === storybookId || item.id === storybookId);
  if (!selected) {
    throw new Error("没有找到绘本。");
  }
  dashboardState.selectedStorybook = selected;
  await loadStudioPages();
  showPage("studio");
  showDashboardToast(`已打开《${selected.title}》。`);
}

async function handleAction(action, target) {
  if (!action) {
    return;
  }
  target.disabled = true;
  try {
    if (action === "save-page") await saveCurrentPage();
    else if (action === "toggle-lock-page") await toggleCurrentPageLock();
    else if (action === "rewrite-page") await rewriteCurrentPage();
    else if (action === "redraw-page") await redrawCurrentPage();
    else if (action === "select-image-output") await selectLatestImageOutput();
    else if (action === "view-review-events") await viewLatestReviewEvents();
    else if (action === "view-costs") await viewGenerationCosts();
    else if (action === "delete-page") await deleteCurrentPage();
    else if (action === "add-page") await addPageToCurrentStorybook();
    else if (action === "export-storybook") await exportStorybook(target.dataset.storybookId);
    else if (action === "share-storybook") await shareStorybook();
    else if (action === "create-child") await createChildFromPrompt();
    else if (action === "edit-child") await editChildFromPrompt(target.dataset.childId);
    else if (action === "open-storybook") await openStorybook(target.dataset.storybookId);
    else if (action === "generate-storybook") await generateStorybookFromCase(target.dataset.caseId);
    else if (action === "rename-role") await renameFirstRole();
    else if (action === "generate-reference-image") await generateReferenceImageForRole(target.dataset.roleKey);
    else if (action === "check-share-scope") await checkShareScope();
    else if (action === "view-shared-library") await viewSharedLibrary();
    else if (action === "create-family-share") await shareStorybook();
    else if (action === "submit-platform-review") await submitPlatformReview();
    else if (action === "sync-admin-context") await syncAdminContext();
    else if (action === "manage-organization") await manageOrganization();
    else if (action === "manage-templates") await manageTemplates();
    else if (action === "manage-parent-intake") await manageParentIntake();
    else if (action === "manage-child-photo") await manageChildPhoto();
    else if (action === "manage-visual-subjects") await manageVisualSubjects();
    else if (action === "manage-delivery-details") await manageDeliveryDetails();
    else if (action === "manage-storybook-variants") await manageStorybookVariants();
    else if (action === "manage-image-details") await manageImageDetails();
    else if (action === "show-route-coverage") showRouteCoverage();
    else if (action === "filter-export") {
      const exportFilter = document.querySelector('[data-filter="export"]');
      if (exportFilter) exportFilter.click();
      showPage("queue");
    }
    else if (action === "register-uploaded-asset") await registerUploadedAsset();
  } catch (error) {
    showDashboardToast(error.message);
  } finally {
    target.disabled = false;
  }
}

navLinks.forEach((link) => {
  link.addEventListener("click", (event) => {
    event.preventDefault();
    showPage(link.dataset.pageLink);
  });
});

document.addEventListener("click", (event) => {
  const target = event.target.closest("[data-action]");
  if (!target) {
    return;
  }
  event.preventDefault();
  handleAction(target.dataset.action, target);
});

filterButtons.forEach((button) => {
  button.addEventListener("click", () => {
    const filter = button.dataset.filter;
    filterButtons.forEach((item) => item.classList.toggle("active", item === button));
    renderWorkList(filter);
    showDashboardToast(filter === "all" ? "已显示全部待办。" : "已更新待办筛选。");
  });
});

if (studioTitleInput && studioPreviewTitle) {
  studioTitleInput.addEventListener("input", () => {
    studioPreviewTitle.textContent = studioTitleInput.value || "未填写标题";
  });
}

if (studioCopyInput && studioPreviewCopy) {
  studioCopyInput.addEventListener("input", () => {
    studioPreviewCopy.textContent = studioCopyInput.value || "未填写正文";
  });
}

choiceButtons.forEach((button) => {
  button.addEventListener("click", () => {
    choiceButtons.forEach((item) => item.classList.toggle("active", item === button));
  });
});

bindToastButtons();
showPage(window.location.hash.replace("#", "") || "today", false);
loadDashboardData();
