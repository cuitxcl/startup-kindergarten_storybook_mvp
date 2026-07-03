const navToggle = document.querySelector(".nav-toggle");
const mobileNav = document.querySelector("#mobile-nav");
const navLinks = Array.from(document.querySelectorAll(".site-nav a, .mobile-nav a"));
const sectionNavLinks = navLinks.filter((link) => link.getAttribute("href").startsWith("#"));
const toast = document.querySelector("#toast");
const backendStatusPill = document.querySelector("#backend-status-pill");
const backendStatusTitle = document.querySelector("#backend-status-title");
const backendStatusCopy = document.querySelector("#backend-status-copy");
const checkBackendButton = document.querySelector("#check-backend");

function showToast(message) {
  toast.textContent = message;
  toast.classList.remove("is-hidden");
  clearTimeout(showToast.timer);
  showToast.timer = setTimeout(() => {
    toast.classList.add("is-hidden");
  }, 2600);
}

function closeMobileNav() {
  mobileNav.classList.remove("open");
  navToggle.setAttribute("aria-expanded", "false");
}

navToggle.addEventListener("click", () => {
  const nextOpen = !mobileNav.classList.contains("open");
  mobileNav.classList.toggle("open", nextOpen);
  navToggle.setAttribute("aria-expanded", String(nextOpen));
});

navLinks.forEach((link) => {
  link.addEventListener("click", () => {
    closeMobileNav();
  });
});

const sections = sectionNavLinks
  .map((link) => document.querySelector(link.getAttribute("href")))
  .filter(Boolean);

function syncActiveNav() {
  let current = sections[0];
  sections.forEach((section) => {
    if (section.getBoundingClientRect().top <= 130) {
      current = section;
    }
  });
  navLinks.forEach((link) => {
    link.classList.toggle("active", link.getAttribute("href") === `#${current.id}`);
  });
}

window.addEventListener("scroll", syncActiveNav, { passive: true });
syncActiveNav();

function setBackendStatus(kind, title, copy) {
  if (!backendStatusPill || !backendStatusTitle || !backendStatusCopy) {
    return;
  }
  backendStatusPill.textContent = kind;
  backendStatusTitle.textContent = title;
  backendStatusCopy.textContent = copy;
}

async function refreshBackendStatus() {
  if (!window.KindleleafApi) {
    setBackendStatus("未加载", "API 客户端未加载", "请确认 frontend/api.js 已正确加载。");
    return;
  }
  setBackendStatus("检查中", "正在检查后端连接", `请求 ${window.KindleleafApi.baseUrl}`);
  try {
    await window.KindleleafApi.health();
    const openapi = await window.KindleleafApi.getOpenApi();
    const pathCount = Object.keys(openapi.paths || {}).length;
    setBackendStatus("已连接", "后端接口可访问", `已读取 OpenAPI 文档，当前登记 ${pathCount} 组路径。`);
    showToast("后端连接正常。");
  } catch (error) {
    setBackendStatus("连接失败", "无法访问后端接口", error.message);
    showToast(error.message);
  }
}

checkBackendButton?.addEventListener("click", refreshBackendStatus);
refreshBackendStatus();
