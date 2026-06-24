const navToggle = document.querySelector(".nav-toggle");
const mobileNav = document.querySelector("#mobile-nav");
const navLinks = Array.from(document.querySelectorAll(".site-nav a, .mobile-nav a"));
const demoForm = document.querySelector("#demo-form");
const toast = document.querySelector("#toast");
const formNote = document.querySelector("#form-note");

function showToast(message) {
  toast.textContent = message;
  toast.classList.remove("is-hidden");
  clearTimeout(showToast.timer);
  showToast.timer = setTimeout(() => {
    toast.classList.add("is-hidden");
  }, 2600);
}

function setError(name, message) {
  const error = document.querySelector(`[data-error-for="${name}"]`);
  if (error) {
    error.textContent = message;
  }
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

const sections = navLinks
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

demoForm.addEventListener("submit", (event) => {
  event.preventDefault();
  const data = new FormData(demoForm);
  const school = String(data.get("school") || "").trim();
  const acceptedPrivacy = data.get("privacy") === "on";
  let valid = true;

  setError("school", "");
  setError("privacy", "");

  if (!school) {
    setError("school", "请填写园所或班级，便于生成演示入口。");
    valid = false;
  }

  if (!acceptedPrivacy) {
    setError("privacy", "请先确认不会提交儿童完整姓名、照片或家庭隐私。");
    valid = false;
  }

  if (!valid) {
    return;
  }

  formNote.textContent = `${school} 的演示请求已创建，建议先从“${data.get("theme")}”开始。`;
  demoForm.reset();
  showToast("演示请求已保存到当前页面状态。");
});
