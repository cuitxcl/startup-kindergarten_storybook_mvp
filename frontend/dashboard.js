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

let dashboardState = {
  contentItems: [],
  children: [],
  cases: [],
  selectedStorybook: null,
  pages: [],
  selectedPage: null,
  workItems: [],
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
  const pagesResponse = await window.KindleleafApi.listPages(selected.storybook_id);
  renderPages(pagesResponse);
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
    else if (action === "delete-page") await deleteCurrentPage();
    else if (action === "add-page") await addPageToCurrentStorybook();
    else if (action === "export-storybook") await exportStorybook(target.dataset.storybookId);
    else if (action === "share-storybook") await shareStorybook();
    else if (action === "create-child") await createChildFromPrompt();
    else if (action === "edit-child") await editChildFromPrompt(target.dataset.childId);
    else if (action === "open-storybook") await openStorybook(target.dataset.storybookId);
    else if (action === "generate-storybook") await generateStorybookFromCase(target.dataset.caseId);
    else if (action === "filter-export") {
      const exportFilter = document.querySelector('[data-filter="export"]');
      if (exportFilter) exportFilter.click();
      showPage("queue");
    }
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
