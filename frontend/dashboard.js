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
      kind: "profile",
      badge: "待补档",
      title: `${child.name}的孩子档案`,
      copy: `${text(child.age, "-")} 岁 · ${text(child.teaching_focus, "待补充教学关注")} · ${statusLabel(child.profile_completion_status)}`,
      meta: "生成前",
      action: "补充",
    }));

  const storyItems = dashboardState.cases.slice(0, 4).map((item) => ({
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
    actions.append(rowButton(item.action, `已进入${item.badge}处理：${item.title}`));
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
    row.append(rowButton("打开", `已打开《${item.title}》。`));
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
    row.append(rowButton(child.profile_completion_status === "complete" ? "编辑" : "补充", `已定位到${child.name}档案。`));
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
}

function renderPages(pagesResponse) {
  const storyPages = pagesResponse.items || [];
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
    button.innerHTML = `<span>${page.page_role === "cover" ? "Cover" : `Page ${escapeHtml(page.page_number)}`}</span><strong>${escapeHtml(page.page_title || page.body_text)}</strong>`;
    button.addEventListener("click", () => selectStoryPage(button));
    pageStripList.append(button);
  });

  selectStoryPage(pageStripList.querySelector(".page-thumb"));
}

async function loadStudioPages() {
  const selected = dashboardState.contentItems[0];
  dashboardState.selectedStorybook = selected;
  if (!selected) {
    renderPages({ items: [] });
    return;
  }

  setText("#storybook-roles-label", selected.child ? `${selected.child.name} · ${selected.theme}` : selected.theme);
  const pagesResponse = await window.KindleleafApi.listPages(selected.storybook_id);
  renderPages(pagesResponse);
}

async function loadDashboardData() {
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
    showDashboardToast("已从后端接口同步控制台数据。");
  } catch (error) {
    showDashboardToast(`后端接口连接失败：${error.message}`);
    const message = "无法连接后端接口，请确认 Loco 服务已启动，或使用 ?api=http://127.0.0.1:5150 指定地址。";
    if (workList) {
      workList.replaceChildren(Object.assign(document.createElement("article"), { className: "empty-state", textContent: message }));
    }
  }
}

navLinks.forEach((link) => {
  link.addEventListener("click", (event) => {
    event.preventDefault();
    showPage(link.dataset.pageLink);
  });
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
