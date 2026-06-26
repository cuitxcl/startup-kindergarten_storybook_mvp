const dashboardToast = document.querySelector("#toast");
const actionButtons = Array.from(document.querySelectorAll("[data-toast]"));
const navLinks = Array.from(document.querySelectorAll("[data-page-link]"));
const pages = Array.from(document.querySelectorAll("[data-page]"));
const filterButtons = Array.from(document.querySelectorAll("[data-filter]"));
const workRows = Array.from(document.querySelectorAll(".work-row"));
const pageThumbs = Array.from(document.querySelectorAll(".page-thumb"));
const studioPageTitle = document.querySelector("#studio-page-title");
const studioPreviewTitle = document.querySelector("#studio-preview-title");
const studioPreviewCopy = document.querySelector("#studio-preview-copy");
const studioTitleInput = document.querySelector("#studio-title-input");
const studioCopyInput = document.querySelector("#studio-copy-input");
const choiceButtons = Array.from(document.querySelectorAll(".choice-grid button"));

function showDashboardToast(message) {
  dashboardToast.textContent = message;
  dashboardToast.classList.remove("is-hidden");
  clearTimeout(showDashboardToast.timer);
  showDashboardToast.timer = setTimeout(() => {
    dashboardToast.classList.add("is-hidden");
  }, 2600);
}

actionButtons.forEach((button) => {
  button.addEventListener("click", () => {
    showDashboardToast(button.dataset.toast);
  });
});

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
    workRows.forEach((row) => {
      const visible = filter === "all" || row.dataset.kind === filter;
      row.classList.toggle("is-hidden", !visible);
    });
    showDashboardToast(filter === "all" ? "已显示全部待办。" : "已更新待办筛选。");
  });
});

function selectStoryPage(thumb) {
  pageThumbs.forEach((item) => item.classList.toggle("active", item === thumb));

  if (!studioPageTitle || !studioPreviewTitle || !studioPreviewCopy || !studioTitleInput || !studioCopyInput) {
    return;
  }

  studioPageTitle.textContent = thumb.dataset.pageTitle;
  studioPreviewTitle.textContent = thumb.dataset.pageCopy;
  studioPreviewCopy.textContent = thumb.dataset.pageNote;
  studioTitleInput.value = thumb.dataset.pageCopy;
  studioCopyInput.value = thumb.dataset.pageNote;
}

pageThumbs.forEach((thumb) => {
  thumb.addEventListener("click", () => {
    selectStoryPage(thumb);
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

showPage(window.location.hash.replace("#", "") || "today", false);
