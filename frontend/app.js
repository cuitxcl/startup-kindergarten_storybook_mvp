const navToggle = document.querySelector(".nav-toggle");
const mobileNav = document.querySelector("#mobile-nav");
const navLinks = Array.from(document.querySelectorAll(".site-nav a, .mobile-nav a"));
const sectionNavLinks = navLinks.filter((link) => link.getAttribute("href").startsWith("#"));
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

function themeToCaseTheme(theme) {
  if (theme.includes("分享")) {
    return "分享合作";
  }
  if (theme.includes("午睡")) {
    return "生活常规";
  }
  if (theme.includes("排队")) {
    return "生活常规";
  }
  return "入园适应";
}

async function ensureDemoSession() {
  if (window.KindleleafApi.currentToken()) {
    try {
      await window.KindleleafApi.me();
      return;
    } catch (_error) {
      window.KindleleafApi.clearToken();
    }
  }
  await window.KindleleafApi.login({
    identifier: "teacher@example.com",
    password: "password123",
  });
}

demoForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const data = new FormData(demoForm);
  const school = String(data.get("school") || "").trim();
  const theme = String(data.get("theme") || "勇敢去幼儿园");
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

  try {
    await ensureDemoSession();
    const child = await window.KindleleafApi.createChild({
      name: "演示孩子",
      nickname: "乐乐",
      age: 5,
      age_group: "5-6",
      interest_tags: ["积木"],
      teacher_observation_tags: ["愿意尝试"],
      teaching_focus: theme,
    });
    let cases = await window.KindleleafApi.listCasesByTheme(themeToCaseTheme(theme));
    if (!cases.items?.length) {
      cases = await window.KindleleafApi.listCases();
    }
    const selectedCase = cases.items?.[0];
    if (!selectedCase) {
      throw new Error("当前主题没有可用母本");
    }
    const generated = await window.KindleleafApi.generateStorybook({
      content_type: "custom_storybook",
      child_id: child.id,
      case_storybook_id: selectedCase.id,
      title_override: `乐乐的${selectedCase.title}`,
      style_id: "soft-colored-pencil",
      reading_age_group: "5-6",
      teaching_goal: theme,
      generation_options: {
        source: "homepage_demo",
        school,
      },
    });
    formNote.textContent = `${school} 的演示绘本已创建：《${generated.storybook.title}》。`;
    demoForm.reset();
    showToast("演示请求已提交到后端。");
  } catch (error) {
    formNote.textContent = `提交失败：${error.message}`;
    showToast(error.message);
  }
});
