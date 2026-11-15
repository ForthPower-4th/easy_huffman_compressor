<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import pickIcon from "./assets/icon.svg";
import {
  PICK_OUTPUT_PATH_COMMAND,
  commandName,
  isTauriRuntime,
  pickOutputPath,
  runOperation,
  type Mode,
  type OperationReport,
} from "./lib/bridge";

type Status = "idle" | "running" | "done" | "error";

const mode = ref<Mode>("compress");
const inputPath = ref("");
const outputPath = ref("");
const status = ref<Status>("idle");
const progress = ref(0);
const report = ref<OperationReport | null>(null);
const errorText = ref("");
const pickerError = ref("");
const dragging = ref(false);
const isPicking = ref(false);
const inputEl = ref<HTMLInputElement | null>(null);
const isTauri = isTauriRuntime();

const isRunning = computed(() => status.value === "running");
const canRun = computed(
  () =>
    inputPath.value.trim().length > 0 &&
    outputPath.value.trim().length > 0 &&
    !isRunning.value,
);
const actionLabel = computed(() =>
  mode.value === "compress" ? "压缩" : "解压",
);
const headline = computed(() =>
  mode.value === "compress" ? "压缩文件" : "解压文件",
);
const subline = computed(() =>
  mode.value === "compress"
    ? "Huffman 无损压缩，输出 .huff 文件。"
    : "选择 .huff 文件，还原出原始内容。",
);
const fileName = computed(() => baseName(inputPath.value.trim()));
const fileExt = computed(() => extensionOf(fileName.value));
const outputFileExt = computed(() =>
  extensionOf(baseName(outputPath.value.trim())),
);
const expectedOutputName = computed(() =>
  baseName(defaultOutputPath(inputPath.value, mode.value)),
);
const inputLabel = computed(() =>
  mode.value === "compress" ? "原始" : "压缩包",
);
const outputLabel = computed(() =>
  mode.value === "compress" ? "压缩后" : "解压后",
);

const ratioNote = computed(() => {
  if (!report.value) return "";
  const { inputBytes, outputBytes } = report.value;
  // a malformed report must not turn into "NaN%" on screen
  if (!Number.isFinite(inputBytes) || !Number.isFinite(outputBytes)) return "";
  if (inputBytes <= 0) return "";
  const ratio = outputBytes / inputBytes;
  if (mode.value === "compress") {
    return ratio <= 1
      ? `体积减少 ${((1 - ratio) * 100).toFixed(1)}%`
      : `体积增加了 ${((ratio - 1) * 100).toFixed(1)}%`;
  }
  return `还原为压缩包的 ${ratio.toFixed(2)} 倍`;
});

const resultNote = computed(() => {
  if (!report.value) return "";
  const time = `用时 ${formatDuration(report.value.elapsedMs)}`;
  return ratioNote.value ? `${ratioNote.value} · ${time}` : time;
});

function baseName(path: string): string {
  if (!path) return "";
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function extensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) return "FILE";
  return name.slice(dot + 1, dot + 5).toUpperCase();
}

function replaceExtension(path: string, extension: string): string {
  const trimmed = path.trim();
  if (!trimmed) return "";
  const separator = Math.max(
    trimmed.lastIndexOf("\\"),
    trimmed.lastIndexOf("/"),
  );
  const directory = trimmed.slice(0, separator + 1);
  const name = trimmed.slice(separator + 1);
  // 只去掉文件名最后一个点及其后的后缀，目录名里的点保持不变
  const dot = name.lastIndexOf(".");
  const stem = dot > 0 ? name.slice(0, dot) : name;
  return `${directory}${stem}.${extension}`;
}

function defaultOutputPath(path: string, currentMode: Mode): string {
  return replaceExtension(path, currentMode === "compress" ? "huff" : "txt");
}

watch(
  inputPath,
  (path) => {
    outputPath.value = defaultOutputPath(path, mode.value);
    pickerError.value = "";
  },
  { flush: "sync" },
);

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 100 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`;
}

function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "—";
  if (ms < 1000) return `${Math.max(1, Math.round(ms))} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(ms < 10_000 ? 2 : 1)} s`;
  const seconds = Math.round(ms / 1000);
  return `${Math.floor(seconds / 60)} 分 ${seconds % 60} 秒`;
}

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, Math.round(value)));
}

function resetOutcome(): void {
  status.value = "idle";
  progress.value = 0;
  report.value = null;
  errorText.value = "";
}

function focusInput(): void {
  inputEl.value?.focus();
}

function setMode(next: Mode): void {
  if (isRunning.value || next === mode.value) return;
  mode.value = next;
  outputPath.value = defaultOutputPath(inputPath.value, next);
  pickerError.value = "";
  resetOutcome();
}

function acceptPath(path: string): void {
  if (isRunning.value) return;
  inputPath.value = path;
  outputPath.value = defaultOutputPath(path, mode.value);
  resetOutcome();
}

function clearFile(): void {
  if (isRunning.value) return;
  inputPath.value = "";
  outputPath.value = "";
  resetOutcome();
  focusInput();
}

function onOutputInput(): void {
  pickerError.value = "";
  resetOutcome();
}

async function chooseOutputPath(): Promise<void> {
  if (isRunning.value || isPicking.value) return;
  const expectedName = expectedOutputName.value;
  if (!expectedName) {
    pickerError.value = "请先选择输入文件，再选择输出路径。";
    return;
  }
  if (!isTauri) {
    pickerError.value =
      "浏览器预览无法打开文件选择器，请用 pnpm tauri dev 打开桌面窗口。";
    return;
  }

  pickerError.value = "";
  isPicking.value = true;
  try {
    const picked = await pickOutputPath(expectedName);
    if (!picked) return;
    outputPath.value = picked;
    resetOutcome();
  } catch (error) {
    pickerError.value = describeError(error, PICK_OUTPUT_PATH_COMMAND);
  } finally {
    isPicking.value = false;
  }
}

function describeError(
  error: unknown,
  command = commandName(mode.value),
): string {
  const message = error instanceof Error ? error.message : String(error ?? "");
  if (/not found/i.test(message)) {
    return `命令 ${command} 尚未注册。实现后记得加入 generate_handler!。`;
  }
  if (/not allowed|permission/i.test(message)) {
    return `权限不足：${message}。自定义命令一般不需要 capability，请检查是否调用了受权限保护的插件 API。`;
  }
  return message || "未知错误，请打开开发者工具查看控制台。";
}

async function run(): Promise<void> {
  if (!canRun.value) return;

  if (!isTauri) {
    status.value = "error";
    errorText.value =
      "浏览器预览无法执行文件操作，请用 pnpm tauri dev 打开桌面窗口。";
    return;
  }

  resetOutcome();
  status.value = "running";

  try {
    report.value = await runOperation(mode.value, {
      inputPath: inputPath.value.trim(),
      outputPath: outputPath.value.trim(),
      onProgress: (percent) => {
        progress.value = clampPercent(percent);
      },
    });
    status.value = "done";
  } catch (error) {
    status.value = "error";
    errorText.value = describeError(error);
  }
}

let stopDragListener: (() => void) | undefined;

onMounted(async () => {
  if (!isTauri) return;
  try {
    stopDragListener = await getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "enter") {
        dragging.value = true;
      } else if (payload.type === "leave") {
        dragging.value = false;
      } else if (payload.type === "drop") {
        dragging.value = false;
        const [first] = payload.paths;
        if (first) acceptPath(first);
      }
    });
  } catch {
    // 拖拽事件不可用时静默降级，手动输入路径依然可用
  }
});

onUnmounted(() => {
  stopDragListener?.();
});
</script>

<template>
  <div class="app">
    <header class="topbar">
      <div class="brand">
        <span class="brand-mark" aria-hidden="true">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.7"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M8 3v4H4" />
            <path d="M16 3v4h4" />
            <path d="M8 21v-4H4" />
            <path d="M16 21v-4h4" />
          </svg>
        </span>
        <div class="brand-text">
          <span class="brand-name">Huffman</span>
          <span class="brand-sub">本地压缩工具</span>
        </div>
      </div>
      <span class="version">v0.1</span>
    </header>

    <main class="panel">
      <div class="panel-head">
        <h1>{{ headline }}</h1>
        <p>{{ subline }}</p>
      </div>

      <p v-if="!isTauri" class="notice">
        浏览器预览模式：界面可看，文件操作需用 <code>pnpm tauri dev</code> 启动。
      </p>

      <div class="mode-switch" role="group" aria-label="操作模式">
        <span
          class="mode-thumb"
          :class="{ right: mode === 'decompress' }"
          aria-hidden="true"
        ></span>
        <button
          type="button"
          :aria-pressed="mode === 'compress'"
          :disabled="isRunning"
          @click="setMode('compress')"
        >
          压缩
        </button>
        <button
          type="button"
          :aria-pressed="mode === 'decompress'"
          :disabled="isRunning"
          @click="setMode('decompress')"
        >
          解压
        </button>
      </div>

      <section class="field" aria-labelledby="source-label">
        <div class="field-head">
          <span id="source-label">源文件</span>
          <span class="field-hint">拖入窗口，或粘贴完整路径</span>
        </div>

        <div
          class="file-field"
          :class="{ filled: !!inputPath, dragging }"
          @click="focusInput"
        >
          <span class="file-icon" :class="{ ready: !!inputPath }" aria-hidden="true">
            <svg
              v-if="inputPath"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="M20 6 9 17l-5-5" />
            </svg>
            <svg
              v-else
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.7"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
              <path d="M14 3v5h5" />
            </svg>
          </span>

          <input
            id="path"
            ref="inputEl"
            v-model="inputPath"
            type="text"
            spellcheck="false"
            autocomplete="off"
            aria-label="文件路径"
            placeholder="D:\Downloads\example.pdf"
            :disabled="isRunning"
            @input="resetOutcome"
            @keydown.enter="run"
          />

          <span v-if="inputPath" class="ext">{{ fileExt }}</span>

          <button
            v-if="inputPath"
            type="button"
            class="clear"
            aria-label="清除文件"
            :disabled="isRunning"
            @click.stop="clearFile"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
            >
              <path d="M6 6l12 12M18 6 6 18" />
            </svg>
          </button>
        </div>

      </section>

      <section class="field" aria-labelledby="output-label">
        <div class="field-head">
          <span id="output-label">输出文件</span>
          <span class="field-hint">自动生成，可修改或点击左侧选择</span>
        </div>

        <div
          class="file-field output-field"
          :class="{ filled: !!outputPath }"
        >
          <button
            type="button"
            class="pick"
            :disabled="isRunning || isPicking || !expectedOutputName"
            :aria-busy="isPicking"
            :aria-label="expectedOutputName ? '选择输出路径' : '请先选择输入文件'"
            :title="expectedOutputName ? '选择输出路径' : '请先选择输入文件'"
            @click="chooseOutputPath"
          >
            <img :src="pickIcon" alt="" aria-hidden="true" />
          </button>

          <input
            id="output-path"
            v-model="outputPath"
            type="text"
            spellcheck="false"
            autocomplete="off"
            aria-label="输出文件路径"
            placeholder="选择文件"
            :disabled="isRunning"
            @input="onOutputInput"
            @keydown.enter="run"
          />

          <span v-if="outputPath" class="ext">{{ outputFileExt }}</span>
        </div>

        <p v-if="pickerError" class="field-error" role="alert">
          {{ pickerError }}
        </p>
      </section>

      <button type="button" class="primary" :disabled="!canRun" @click="run">
        <span v-if="isRunning" class="spinner" aria-hidden="true"></span>
        <span>{{ isRunning ? `${actionLabel}中…` : `开始${actionLabel}` }}</span>
      </button>

      <div v-if="isRunning" class="progress" aria-live="polite">
        <div class="progress-head">
          <span>正在{{ actionLabel }}，请稍候</span>
          <span class="mono">{{ progress }}%</span>
        </div>
        <div
          class="track"
          role="progressbar"
          :aria-valuenow="progress"
          aria-valuemin="0"
          aria-valuemax="100"
        >
          <div class="fill" :style="{ width: `${progress}%` }"></div>
        </div>
      </div>

      <Transition name="pop">
        <div v-if="status === 'done' && report" class="result" role="status">
          <div class="result-head">
            <span class="ok-icon" aria-hidden="true">
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path d="M20 6 9 17l-5-5" />
              </svg>
            </span>
            <div class="result-copy">
              <p class="result-title">{{ actionLabel }}完成</p>
              <p class="result-sub">{{ resultNote }}</p>
            </div>
          </div>

          <div class="sizes">
            <div class="size">
              <span>{{ inputLabel }}</span>
              <strong>{{ formatBytes(report.inputBytes) }}</strong>
            </div>
            <span class="arrow" aria-hidden="true">→</span>
            <div class="size">
              <span>{{ outputLabel }}</span>
              <strong>{{ formatBytes(report.outputBytes) }}</strong>
            </div>
          </div>
        </div>
      </Transition>

      <Transition name="pop">
        <div v-if="status === 'error'" class="alert" role="alert">
          <p class="alert-title">{{ actionLabel }}失败</p>
          <p class="alert-text">{{ errorText }}</p>
        </div>
      </Transition>
    </main>

    <footer class="foot">
      <span class="dot" aria-hidden="true"></span>
      文件只在本机处理，不会上传
    </footer>
  </div>
</template>

<style scoped>
.app {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 36px 22px 20px;
  gap: 18px;
}

.topbar {
  width: min(680px, 100%);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.brand {
  display: flex;
  align-items: center;
  gap: 11px;
}

.brand-mark {
  width: 38px;
  height: 38px;
  display: grid;
  place-items: center;
  border-radius: 12px;
  color: var(--accent);
  background: linear-gradient(
    135deg,
    rgba(94, 234, 212, 0.2),
    rgba(96, 165, 250, 0.16)
  );
  border: 1px solid rgba(94, 234, 212, 0.32);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.12);
}

.brand-mark svg {
  width: 20px;
  height: 20px;
}

.brand-text {
  display: flex;
  flex-direction: column;
  line-height: 1.2;
}

.brand-name {
  font-size: 15px;
  font-weight: 650;
  letter-spacing: 0.2px;
}

.brand-sub {
  margin-top: 2px;
  font-size: 11.5px;
  color: var(--muted);
}

.version {
  padding: 4px 9px;
  border-radius: 999px;
  border: 1px solid var(--line);
  background: rgba(255, 255, 255, 0.03);
  color: var(--muted);
  font-family: inherit;
  font-size: 11px;
}

.panel {
  width: min(680px, 100%);
  padding: 28px 30px 30px;
  border-radius: 22px;
  border: 1px solid var(--line);
  background: linear-gradient(
    180deg,
    rgba(255, 255, 255, 0.055),
    rgba(255, 255, 255, 0.022)
  );
  box-shadow:
    0 30px 80px -32px rgba(0, 0, 0, 0.9),
    inset 0 1px 0 rgba(255, 255, 255, 0.07);
  backdrop-filter: blur(18px);
}

.panel-head h1 {
  margin: 0 0 6px;
  font-size: 24px;
  font-weight: 650;
  letter-spacing: -0.3px;
}

.panel-head p {
  margin: 0;
  color: var(--muted);
  font-size: 13px;
}

.notice {
  margin: 16px 0 0;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid rgba(251, 191, 36, 0.22);
  background: rgba(251, 191, 36, 0.06);
  color: #e9c46a;
  font-size: 12px;
  line-height: 1.5;
}

.notice code {
  font-family: inherit;
  font-size: 11.5px;
  color: #f6d58b;
}

.mode-switch {
  position: relative;
  display: grid;
  grid-template-columns: 1fr 1fr;
  margin: 20px 0 18px;
  padding: 4px;
  border-radius: 12px;
  border: 1px solid var(--line);
  background: rgba(3, 7, 11, 0.45);
}

.mode-thumb {
  position: absolute;
  top: 4px;
  left: 4px;
  width: calc(50% - 4px);
  height: calc(100% - 8px);
  border-radius: 9px;
  background: linear-gradient(
    135deg,
    rgba(94, 234, 212, 0.18),
    rgba(96, 165, 250, 0.14)
  );
  border: 1px solid rgba(94, 234, 212, 0.3);
  transition: transform 0.26s cubic-bezier(0.2, 0.8, 0.2, 1);
}

.mode-thumb.right {
  transform: translateX(100%);
}

.mode-switch button {
  position: relative;
  z-index: 1;
  padding: 9px 0;
  border: 0;
  background: none;
  border-radius: 9px;
  color: var(--muted);
  font-size: 13px;
  font-weight: 550;
  cursor: pointer;
  transition: color 0.2s ease;
}

.mode-switch button[aria-pressed="true"] {
  color: var(--text);
}

.mode-switch button:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.field-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  margin-bottom: 8px;
  font-size: 12px;
}

.field-head span:first-child {
  color: var(--text);
  font-weight: 550;
}

.field-hint {
  color: var(--faint);
}

.field + .field {
  margin-top: 16px;
}

.file-field {
  display: flex;
  align-items: center;
  gap: 11px;
  height: 58px;
  padding: 0 12px;
  border-radius: 13px;
  border: 1px dashed rgba(255, 255, 255, 0.16);
  background: rgba(3, 7, 11, 0.45);
  cursor: text;
  transition:
    border-color 0.2s ease,
    background 0.2s ease,
    box-shadow 0.2s ease;
}

.file-field:hover {
  border-color: rgba(255, 255, 255, 0.24);
}

.file-field.filled {
  border-style: solid;
  border-color: rgba(255, 255, 255, 0.13);
}

.file-field.dragging {
  border-color: var(--accent);
  background: rgba(94, 234, 212, 0.07);
  box-shadow: 0 0 0 4px rgba(94, 234, 212, 0.1);
}

.file-field:focus-within {
  border-style: solid;
  border-color: rgba(94, 234, 212, 0.55);
  box-shadow: 0 0 0 4px rgba(94, 234, 212, 0.08);
}

.file-icon {
  flex: none;
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  border-radius: 10px;
  color: var(--faint);
  background: rgba(255, 255, 255, 0.04);
  transition:
    color 0.2s ease,
    background 0.2s ease;
}

.file-icon svg {
  width: 18px;
  height: 18px;
}

.file-icon.ready {
  color: var(--accent);
  background: rgba(94, 234, 212, 0.1);
}

.file-field input {
  flex: 1;
  min-width: 0;
  border: 0;
  outline: 0;
  background: none;
  color: var(--text);
  font-family: inherit;
  font-size: 12.5px;
  letter-spacing: 0.1px;
}

.file-field input::placeholder {
  color: var(--faint);
}

.file-field input:disabled {
  opacity: 0.65;
}

.ext {
  flex: none;
  padding: 3px 6px;
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: var(--muted);
  font-family: inherit;
  font-size: 10px;
  letter-spacing: 0.6px;
}

.clear {
  flex: none;
  width: 26px;
  height: 26px;
  display: grid;
  place-items: center;
  border: 0;
  border-radius: 8px;
  background: none;
  color: var(--faint);
  cursor: pointer;
  transition:
    color 0.15s ease,
    background 0.15s ease;
}

.clear svg {
  width: 14px;
  height: 14px;
}

.clear:hover:not(:disabled) {
  color: var(--text);
  background: rgba(255, 255, 255, 0.07);
}

.clear:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.pick {
  position: relative;
  flex: none;
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  padding: 0;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.04);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.06);
  cursor: pointer;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease,
    transform 0.15s ease,
    opacity 0.2s ease;
}

/* 暗色底下的高亮渐变，只在 hover 时淡入 */
.pick::before {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background: linear-gradient(135deg, #6ee7d8, #7dd3fc);
  opacity: 0;
  transition: opacity 0.2s ease;
  pointer-events: none;
}

.pick img {
  position: relative;
  z-index: 1;
  width: 18px;
  height: 18px;
  object-fit: contain;
  pointer-events: none;
  filter: brightness(0) invert(1);
  opacity: 0.45;
  transition: opacity 0.2s ease;
}

.pick:hover:not(:disabled) {
  transform: translateY(-1px);
  border-color: rgba(255, 255, 255, 0.32);
  box-shadow:
    0 8px 18px -10px rgba(94, 234, 212, 0.9),
    inset 0 1px 0 rgba(255, 255, 255, 0.4);
}

.pick:hover:not(:disabled)::before {
  opacity: 1;
}

.pick:hover:not(:disabled) img {
  opacity: 0.95;
}

.pick:active:not(:disabled) {
  transform: translateY(0) scale(0.97);
}

.pick:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.field-error {
  margin: 8px 2px 0;
  color: #fca5a5;
  font-size: 11.5px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}

.primary {
  width: 100%;
  height: 48px;
  margin-top: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 9px;
  border: 0;
  border-radius: 12px;
  background: linear-gradient(135deg, #6ee7d8, #7dd3fc);
  color: #04110d;
  font-size: 14px;
  font-weight: 650;
  letter-spacing: 0.2px;
  cursor: pointer;
  box-shadow: 0 12px 26px -12px rgba(94, 234, 212, 0.55);
  transition:
    transform 0.15s ease,
    box-shadow 0.2s ease,
    opacity 0.2s ease;
}

.primary:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 16px 32px -12px rgba(94, 234, 212, 0.65);
}

.primary:active:not(:disabled) {
  transform: translateY(0);
}

.primary:disabled {
  opacity: 0.38;
  cursor: not-allowed;
  box-shadow: none;
}

.spinner {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  border: 2px solid rgba(4, 17, 13, 0.22);
  border-top-color: #04110d;
  animation: spin 0.7s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.progress {
  margin-top: 16px;
}

.progress-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
  font-size: 11.5px;
  color: var(--muted);
}

.mono {
  font-family: inherit;
  font-variant-numeric: tabular-nums;
}

.track {
  height: 6px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.07);
  overflow: hidden;
}

.fill {
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #5eead4, #60a5fa);
  transition: width 0.22s ease;
}

.result {
  margin-top: 18px;
  padding: 16px;
  border-radius: 14px;
  border: 1px solid rgba(52, 211, 153, 0.26);
  background: rgba(52, 211, 153, 0.055);
}

.result-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.ok-icon {
  flex: none;
  width: 27px;
  height: 27px;
  display: grid;
  place-items: center;
  border-radius: 999px;
  color: var(--ok);
  background: rgba(52, 211, 153, 0.14);
  border: 1px solid rgba(52, 211, 153, 0.34);
}

.ok-icon svg {
  width: 15px;
  height: 15px;
}

.result-copy {
  min-width: 0;
}

.result-title {
  margin: 0;
  font-size: 13.5px;
  font-weight: 600;
}

.result-sub {
  margin: 3px 0 0;
  font-size: 11.5px;
  color: var(--muted);
}

.sizes {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-top: 14px;
  padding-top: 13px;
  border-top: 1px dashed rgba(255, 255, 255, 0.1);
}

.size {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.size span {
  font-size: 10.5px;
  color: var(--muted);
}

.size strong {
  font-size: 18px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.2px;
}

.arrow {
  color: var(--faint);
  font-size: 13px;
}

.alert {
  margin-top: 18px;
  padding: 14px 15px;
  border-radius: 12px;
  border: 1px solid rgba(248, 113, 113, 0.3);
  background: rgba(248, 113, 113, 0.06);
}

.alert-title {
  margin: 0 0 4px;
  color: #fca5a5;
  font-size: 12.5px;
  font-weight: 600;
}

.alert-text {
  margin: 0;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.55;
  overflow-wrap: anywhere;
}

.foot {
  display: flex;
  align-items: center;
  gap: 7px;
  color: #5b6b7f;
  font-size: 11.5px;
}

.dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--ok);
  box-shadow: 0 0 8px rgba(52, 211, 153, 0.7);
}

.primary:focus-visible,
.mode-switch button:focus-visible,
.clear:focus-visible,
.pick:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.pop-enter-active,
.pop-leave-active {
  transition:
    opacity 0.22s ease,
    transform 0.22s ease;
}

.pop-enter-from,
.pop-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

@media (max-width: 560px) {
  .app {
    padding: 22px 14px 16px;
  }

  .panel {
    padding: 22px;
    border-radius: 18px;
  }

  .panel-head h1 {
    font-size: 21px;
  }
}

@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
  }
}
</style>
