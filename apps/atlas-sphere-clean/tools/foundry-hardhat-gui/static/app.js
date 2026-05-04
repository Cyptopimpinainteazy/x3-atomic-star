const state = {
  environment: {},
  install: { sections: [] },
  ganache: null,
  projects: [],
  tasks: [],
  selectedTruffleIndex: -1,
};

const actionsByTool = {
  foundry: ["build", "test", "clean", "node"],
  hardhat: ["compile", "test", "node"],
};

const envStatus = document.getElementById("envStatus");
const installGrid = document.getElementById("installGrid");
const projectGrid = document.getElementById("projectGrid");
const taskList = document.getElementById("taskList");

const projectCardTemplate = document.getElementById("projectCardTemplate");
const taskItemTemplate = document.getElementById("taskItemTemplate");

const logDialog = document.getElementById("logDialog");
const logTitle = document.getElementById("logTitle");
const logContent = document.getElementById("logContent");
const closeLogButton = document.getElementById("closeLogButton");

const refreshProjectsButton = document.getElementById("refreshProjectsButton");
const refreshGanacheButton = document.getElementById("refreshGanacheButton");
const saveGanacheProfileButton = document.getElementById("saveGanacheProfileButton");
const startGanacheButton = document.getElementById("startGanacheButton");
const stopGanacheButton = document.getElementById("stopGanacheButton");
const addTruffleProjectButton = document.getElementById("addTruffleProjectButton");
const removeTruffleProjectButton = document.getElementById("removeTruffleProjectButton");

const workspaceName = document.getElementById("workspaceName");
const rpcHost = document.getElementById("rpcHost");
const rpcPort = document.getElementById("rpcPort");
const chainId = document.getElementById("chainId");
const defaultBalance = document.getElementById("defaultBalance");
const totalAccounts = document.getElementById("totalAccounts");
const autoGenerateMnemonic = document.getElementById("autoGenerateMnemonic");
const mnemonic = document.getElementById("mnemonic");
const lockAccounts = document.getElementById("lockAccounts");
const gasLimit = document.getElementById("gasLimit");
const gasPrice = document.getElementById("gasPrice");
const hardfork = document.getElementById("hardfork");
const outputLogsToFile = document.getElementById("outputLogsToFile");
const logFileDirectory = document.getElementById("logFileDirectory");
const verboseLogs = document.getElementById("verboseLogs");
const googleAnalytics = document.getElementById("googleAnalytics");
const truffleProjectInput = document.getElementById("truffleProjectInput");
const truffleProjectList = document.getElementById("truffleProjectList");
const mnemonicWarning = document.getElementById("mnemonicWarning");
const analyticsNote = document.getElementById("analyticsNote");
const ganacheStatusText = document.getElementById("ganacheStatusText");
const ganacheCapabilityText = document.getElementById("ganacheCapabilityText");
const ganacheCommandPreview = document.getElementById("ganacheCommandPreview");

function truffleProjectsFromList() {
  return Array.from(truffleProjectList.querySelectorAll("li")).map((li) => li.dataset.path);
}

function requestJson(path, options = {}) {
  return fetch(path, options).then(async (response) => {
    const payload = await response.json().catch(() => ({}));
    if (!response.ok) {
      throw new Error(payload.error || `Request failed (${response.status})`);
    }
    return payload;
  });
}

function createButton(label, className = "button", onClick = null) {
  const button = document.createElement("button");
  button.className = className;
  button.textContent = label;
  if (onClick) {
    button.addEventListener("click", onClick);
  }
  return button;
}

function renderEnvironment() {
  envStatus.innerHTML = "";
  Object.entries(state.environment).forEach(([tool, available]) => {
    const chip = document.createElement("span");
    chip.className = "env-chip";
    chip.dataset.on = String(available);
    chip.textContent = `${tool}: ${available ? "ready" : "missing"}`;
    envStatus.append(chip);
  });
}

function renderInstallCheck() {
  installGrid.innerHTML = "";
  (state.install.sections || []).forEach((section) => {
    const card = document.createElement("article");
    card.className = "install-card";
    const title = document.createElement("h3");
    title.textContent = section.title;

    const badge = document.createElement("span");
    badge.className = "status-pill";
    badge.dataset.status = section.installed ? "succeeded" : "failed";
    badge.textContent = section.installed ? "installed" : "missing";

    const head = document.createElement("div");
    head.className = "project-head";
    head.append(title, badge);
    card.append(head);

    (section.commands || []).forEach((commandText) => {
      const row = document.createElement("div");
      row.className = "install-row";
      const code = document.createElement("code");
      code.textContent = commandText;
      const copyButton = createButton("Copy", "button ghost", () => {
        navigator.clipboard.writeText(commandText).catch(() => {});
      });
      row.append(code, copyButton);
      card.append(row);
    });

    if (section.note) {
      const note = document.createElement("p");
      note.className = "field-note";
      note.textContent = section.note;
      card.append(note);
    }
    installGrid.append(card);
  });
}

function renderProjects() {
  projectGrid.innerHTML = "";
  if (state.projects.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = "No Foundry or Hardhat projects discovered in this workspace.";
    projectGrid.append(empty);
    return;
  }

  state.projects.forEach((project) => {
    const card = projectCardTemplate.content.firstElementChild.cloneNode(true);
    const toolPill = card.querySelector(".tool-pill");
    const nameNode = card.querySelector(".project-name");
    const pathNode = card.querySelector(".project-path");
    const actions = card.querySelector(".actions");

    toolPill.dataset.tool = project.tool;
    toolPill.textContent = project.tool.toUpperCase();
    nameNode.textContent = project.name;
    pathNode.textContent = project.relativePath;

    const actionList = actionsByTool[project.tool] || [];
    actionList.forEach((action) => {
      const button = createButton(action, "button", async () => {
        await runTask({ tool: project.tool, action, cwd: project.path });
      });
      actions.append(button);
    });

    projectGrid.append(card);
  });
}

function statusLabel(task) {
  return task.status === "running" ? "running" : `${task.status} (${task.returnCode ?? "-"})`;
}

function renderTasks() {
  taskList.innerHTML = "";
  if (state.tasks.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = "No tasks yet. Run a command from any project card.";
    taskList.append(empty);
    return;
  }

  state.tasks.forEach((task) => {
    const item = taskItemTemplate.content.firstElementChild.cloneNode(true);
    item.querySelector(".task-title").textContent = `${task.tool} · ${task.action}`;
    item.querySelector(".task-cwd").textContent = task.cwd;
    item.querySelector(".task-command").textContent = task.commandDisplay;

    const status = item.querySelector(".status-pill");
    status.dataset.status = task.status;
    status.textContent = statusLabel(task);

    const controls = item.querySelector(".task-actions");
    controls.append(
      createButton("View Logs", "button", async () => {
        await openLogs(task.id);
      }),
    );
    if (task.isRunning) {
      controls.append(
        createButton("Stop", "button danger", async () => {
          await stopTask(task.id);
        }),
      );
    }
    taskList.append(item);
  });
}

function setMnemonicEditable() {
  mnemonic.disabled = autoGenerateMnemonic.checked;
}

function renderTruffleProjects(projects) {
  truffleProjectList.innerHTML = "";
  projects.forEach((path, index) => {
    const item = document.createElement("li");
    item.textContent = path;
    item.dataset.path = path;
    item.dataset.selected = String(index === state.selectedTruffleIndex);
    item.addEventListener("click", () => {
      state.selectedTruffleIndex = index;
      renderTruffleProjects(truffleProjectsFromList());
    });
    truffleProjectList.append(item);
  });
}

function ensureHardforkOptions(hardforks, selected) {
  hardfork.innerHTML = "";
  (hardforks || []).forEach((fork) => {
    const option = document.createElement("option");
    option.value = fork;
    option.textContent = fork;
    if (fork === selected) {
      option.selected = true;
    }
    hardfork.append(option);
  });
}

function applyGanacheProfile(profile, capabilities, notes) {
  workspaceName.value = profile.workspaceName || "";
  rpcHost.value = profile.rpcHost || "127.0.0.1";
  rpcPort.value = String(profile.rpcPort ?? 7545);
  chainId.value = String(profile.chainId ?? 1337);
  defaultBalance.value = profile.defaultBalance || "1000";
  totalAccounts.value = String(profile.totalAccounts ?? 10);
  autoGenerateMnemonic.checked = Boolean(profile.autoGenerateMnemonic);
  mnemonic.value = profile.mnemonic || "";
  lockAccounts.checked = Boolean(profile.lockAccounts);
  gasLimit.value = profile.gasLimit || "";
  gasPrice.value = profile.gasPrice || "";
  outputLogsToFile.checked = Boolean(profile.outputLogsToFile);
  logFileDirectory.value = profile.logFileDirectory || "logs/ganache";
  verboseLogs.checked = Boolean(profile.verboseLogs);
  googleAnalytics.checked = Boolean(profile.googleAnalytics);
  ensureHardforkOptions(capabilities.hardforks || [], profile.hardfork || "merge");
  mnemonicWarning.textContent = notes?.mnemonicWarning || "";
  analyticsNote.textContent = notes?.analyticsNote || "";
  state.selectedTruffleIndex = -1;
  renderTruffleProjects(profile.truffleProjects || []);
  setMnemonicEditable();
}

function readGanacheProfile() {
  return {
    workspaceName: workspaceName.value.trim(),
    rpcHost: rpcHost.value.trim(),
    rpcPort: Number(rpcPort.value),
    chainId: Number(chainId.value),
    defaultBalance: defaultBalance.value.trim(),
    totalAccounts: Number(totalAccounts.value),
    autoGenerateMnemonic: autoGenerateMnemonic.checked,
    mnemonic: mnemonic.value.trim(),
    lockAccounts: lockAccounts.checked,
    gasLimit: gasLimit.value.trim(),
    gasPrice: gasPrice.value.trim(),
    hardfork: hardfork.value,
    outputLogsToFile: outputLogsToFile.checked,
    logFileDirectory: logFileDirectory.value.trim(),
    verboseLogs: verboseLogs.checked,
    googleAnalytics: googleAnalytics.checked,
    truffleProjects: truffleProjectsFromList(),
  };
}

function renderGanacheStatus(status, capabilities) {
  if (!status) {
    ganacheStatusText.textContent = "Status unavailable";
    ganacheCommandPreview.textContent = "";
    return;
  }
  if (status.running && status.task) {
    ganacheStatusText.textContent = `RUNNING · task ${status.task.id}`;
    ganacheCommandPreview.textContent = status.task.commandDisplay || "";
  } else if (status.task) {
    ganacheStatusText.textContent = `LAST STATUS · ${status.task.status}`;
    ganacheCommandPreview.textContent = status.task.commandDisplay || "";
  } else {
    ganacheStatusText.textContent = "Ganache profile not started";
    ganacheCommandPreview.textContent = "";
  }
  const cli = capabilities?.ganacheCli ? `CLI: ${capabilities.ganacheCliPath}` : "CLI: missing";
  const app = capabilities?.ganacheAppImage ? `AppImage: ${capabilities.ganacheAppImagePath}` : "AppImage: missing";
  ganacheCapabilityText.textContent = `${cli} | ${app}`;
}

async function loadEnvironment() {
  const data = await requestJson("/api/environment");
  state.environment = data.environment || {};
  renderEnvironment();
}

async function loadInstallCheck() {
  state.install = await requestJson("/api/install-check");
  renderInstallCheck();
}

async function loadGanache() {
  state.ganache = await requestJson("/api/ganache");
  applyGanacheProfile(state.ganache.profile, state.ganache.capabilities, state.ganache.notes);
  renderGanacheStatus(state.ganache.status, state.ganache.capabilities);
}

async function saveGanacheProfile() {
  const profile = readGanacheProfile();
  state.ganache = await requestJson("/api/ganache/profile", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ profile }),
  });
  applyGanacheProfile(state.ganache.profile, state.ganache.capabilities, state.ganache.notes);
  renderGanacheStatus(state.ganache.status, state.ganache.capabilities);
}

async function startGanache() {
  const profile = readGanacheProfile();
  state.ganache = await requestJson("/api/ganache/start", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ profile }),
  });
  renderGanacheStatus(state.ganache.status, state.ganache.capabilities);
  await loadTasks();
}

async function stopGanache() {
  state.ganache = await requestJson("/api/ganache/stop", { method: "POST" });
  renderGanacheStatus(state.ganache.status, state.ganache.capabilities);
  await loadTasks();
}

async function loadProjects(forceRefresh = false) {
  const suffix = forceRefresh ? "?refresh=1" : "";
  const data = await requestJson(`/api/projects${suffix}`);
  state.projects = data.projects || [];
  renderProjects();
}

async function loadTasks() {
  const data = await requestJson("/api/tasks");
  state.tasks = data.tasks || [];
  renderTasks();
}

async function runTask({ tool, action, cwd }) {
  await requestJson("/api/tasks", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ tool, action, cwd }),
  });
  await loadTasks();
}

async function stopTask(taskId) {
  await requestJson(`/api/tasks/${taskId}/stop`, { method: "POST" });
  await loadTasks();
}

async function openLogs(taskId) {
  const data = await requestJson(`/api/tasks/${taskId}`);
  const task = data.task;
  logTitle.textContent = `${task.tool} · ${task.action} · ${task.id}`;
  logContent.textContent = (task.logs || []).join("\n");
  logDialog.showModal();
  logContent.scrollTop = logContent.scrollHeight;
}

function addTruffleProject() {
  const path = truffleProjectInput.value.trim();
  if (!path) {
    return;
  }
  const current = truffleProjectsFromList();
  if (!current.includes(path)) {
    current.push(path);
  }
  truffleProjectInput.value = "";
  state.selectedTruffleIndex = current.length - 1;
  renderTruffleProjects(current);
}

function removeSelectedTruffleProject() {
  const current = truffleProjectsFromList();
  if (state.selectedTruffleIndex < 0 || state.selectedTruffleIndex >= current.length) {
    return;
  }
  current.splice(state.selectedTruffleIndex, 1);
  state.selectedTruffleIndex = -1;
  renderTruffleProjects(current);
}

async function bootstrap() {
  await Promise.all([loadEnvironment(), loadInstallCheck(), loadGanache(), loadProjects(), loadTasks()]);
  setInterval(() => {
    loadTasks().catch(() => {});
    loadGanache().catch(() => {});
  }, 1800);
}

refreshProjectsButton.addEventListener("click", async () => {
  try {
    await loadProjects(true);
  } catch (error) {
    alert(error.message);
  }
});

refreshGanacheButton.addEventListener("click", async () => {
  try {
    await loadGanache();
  } catch (error) {
    alert(error.message);
  }
});

saveGanacheProfileButton.addEventListener("click", async () => {
  try {
    await saveGanacheProfile();
  } catch (error) {
    alert(error.message);
  }
});

startGanacheButton.addEventListener("click", async () => {
  try {
    await startGanache();
  } catch (error) {
    alert(error.message);
  }
});

stopGanacheButton.addEventListener("click", async () => {
  try {
    await stopGanache();
  } catch (error) {
    alert(error.message);
  }
});

addTruffleProjectButton.addEventListener("click", addTruffleProject);
removeTruffleProjectButton.addEventListener("click", removeSelectedTruffleProject);
autoGenerateMnemonic.addEventListener("change", setMnemonicEditable);
closeLogButton.addEventListener("click", () => logDialog.close());

bootstrap().catch((error) => {
  alert(`Failed to initialize GUI: ${error.message}`);
});
