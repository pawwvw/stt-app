<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";

// ---------------------------------------------------------------------------
// Типы
// ---------------------------------------------------------------------------
interface ModelInfo {
  id: string;
  name: string;
  file_name: string;
  url: string;
  size_label: string;
  description: string;
  installed: boolean;
}
interface TranscriptionResult {
  text: string;
  srt: string;
  success: boolean;
  error?: string;
  denoised: boolean;
}
interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percent: number;
}
interface TranscribeProgress {
  percent: number;
  stage: string; // "denoise" | "transcribe"
}

// ---------------------------------------------------------------------------
// Состояние
// ---------------------------------------------------------------------------
const models = ref<ModelInfo[]>([]);
const selectedModelId = ref<string>("");
const downloadingId = ref<string>("");
const downloadPercent = ref(0);

const LANGUAGES = [
  { code: "auto", label: "Автоопределение" },
  { code: "ru", label: "Русский" },
  { code: "en", label: "Английский" },
  { code: "de", label: "Немецкий" },
  { code: "fr", label: "Французский" },
  { code: "es", label: "Испанский" },
  { code: "it", label: "Итальянский" },
  { code: "zh", label: "Китайский" },
  { code: "ja", label: "Японский" },
  { code: "tr", label: "Турецкий" },
];
const selectedLanguage = ref("auto");
const denoiseEnabled = ref(true);

const audioFileName = ref<string>("");
const audioFilePath = ref<string>("");
const isProcessing = ref(false);
const transcription = ref("");
const transcriptionSrt = ref("");
const error = ref("");
const notice = ref("");
const showSettings = ref(true);

const transcribePercent = ref(0);
const transcribeStage = ref<string>("transcribe");
const stageLabel = computed(() =>
  transcribeStage.value === "denoise" ? "Шумоподавление…" : "Распознавание…"
);

let unlistenProgress: UnlistenFn | null = null;
let unlistenTranscribe: UnlistenFn | null = null;

const installedModels = computed(() => models.value.filter((m) => m.installed));
const hasInstalledModel = computed(() => installedModels.value.length > 0);
const activeModel = computed(() =>
  models.value.find((m) => m.id === selectedModelId.value)
);

// ---------------------------------------------------------------------------
// Жизненный цикл
// ---------------------------------------------------------------------------
onMounted(async () => {
  await refreshModels();
  unlistenProgress = await listen<DownloadProgress>("download-progress", (e) => {
    if (e.payload.model_id === downloadingId.value) {
      downloadPercent.value = Math.round(e.payload.percent);
    }
  });
  unlistenTranscribe = await listen<TranscribeProgress>("transcribe-progress", (e) => {
    transcribeStage.value = e.payload.stage;
    transcribePercent.value = e.payload.percent;
  });
});
onUnmounted(() => {
  if (unlistenProgress) unlistenProgress();
  if (unlistenTranscribe) unlistenTranscribe();
});

async function refreshModels() {
  try {
    models.value = await invoke<ModelInfo[]>("list_models");
    // Если активная модель не выбрана — выбираем первую установленную
    if (!activeModel.value?.installed) {
      const first = models.value.find((m) => m.installed);
      selectedModelId.value = first ? first.id : "";
    }
  } catch (e) {
    error.value = `Ошибка при загрузке списка моделей: ${e}`;
  }
}

// ---------------------------------------------------------------------------
// Модели
// ---------------------------------------------------------------------------
async function downloadModel(model: ModelInfo) {
  downloadingId.value = model.id;
  downloadPercent.value = 0;
  error.value = "";
  try {
    await invoke<string>("download_model", { modelId: model.id });
    await refreshModels();
    selectedModelId.value = model.id;
  } catch (e) {
    error.value = `Ошибка при скачивании модели «${model.name}»: ${e}`;
  } finally {
    downloadingId.value = "";
    downloadPercent.value = 0;
  }
}

async function deleteModel(model: ModelInfo) {
  try {
    await invoke("delete_model", { modelId: model.id });
    await refreshModels();
  } catch (e) {
    error.value = `Ошибка при удалении модели: ${e}`;
  }
}

// ---------------------------------------------------------------------------
// Файл
// ---------------------------------------------------------------------------
async function openFileDialog() {
  try {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Audio",
          extensions: ["mp3", "wav", "ogg", "m4a", "webm", "flac", "aac"],
        },
      ],
    });
    if (selected && typeof selected === "string") {
      audioFilePath.value = selected;
      audioFileName.value = selected.split(/[/\\]/).pop() || "audio";
      error.value = "";
      transcription.value = "";
    }
  } catch (e) {
    error.value = `Ошибка при выборе файла: ${e}`;
  }
}

function removeFile() {
  audioFileName.value = "";
  audioFilePath.value = "";
  transcription.value = "";
  error.value = "";
  notice.value = "";
}

// ---------------------------------------------------------------------------
// Транскрибация
// ---------------------------------------------------------------------------
async function transcribeAudio() {
  if (!audioFilePath.value) {
    error.value = "Файл не выбран";
    return;
  }
  if (!selectedModelId.value) {
    error.value = "Не выбрана модель";
    return;
  }
  isProcessing.value = true;
  error.value = "";
  notice.value = "";
  transcription.value = "";
  transcriptionSrt.value = "";
  transcribePercent.value = 0;
  transcribeStage.value = "transcribe";
  try {
    const result = await invoke<TranscriptionResult>("transcribe_audio", {
      filePath: audioFilePath.value,
      modelId: selectedModelId.value,
      language: selectedLanguage.value === "auto" ? "auto" : selectedLanguage.value,
      denoise: denoiseEnabled.value,
    });
    if (result.success) {
      transcription.value = result.text;
      transcriptionSrt.value = result.srt;
      if (denoiseEnabled.value && !result.denoised) {
        notice.value =
          "Шумоподавление пропущено: FFmpeg не найден в системе. Текст распознан без предобработки.";
      }
    } else {
      error.value = result.error || "Неизвестная ошибка при расшифровке";
    }
  } catch (e) {
    error.value = `Ошибка при расшифровке: ${e}`;
  } finally {
    isProcessing.value = false;
  }
}

// ---------------------------------------------------------------------------
// Результат: копирование и экспорт
// ---------------------------------------------------------------------------
function copyTranscription() {
  if (transcription.value) navigator.clipboard.writeText(transcription.value);
}

async function exportAs(format: "txt" | "srt" | "md") {
  if (!transcription.value) return;
  let content = transcription.value;
  if (format === "srt") {
    content = transcriptionSrt.value || transcription.value;
  } else if (format === "md") {
    const lang = LANGUAGES.find((l) => l.code === selectedLanguage.value)?.label;
    content =
      `# Расшифровка: ${audioFileName.value}\n\n` +
      `- Модель: ${activeModel.value?.name}\n` +
      `- Язык: ${lang}\n` +
      `- Шумоподавление: ${denoiseEnabled.value ? "да" : "нет"}\n\n` +
      `---\n\n${transcription.value}\n`;
  }
  try {
    const path = await save({
      defaultPath: `${baseName()}.${format}`,
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (path) await writeTextFile(path, content);
  } catch (e) {
    error.value = `Ошибка при экспорте: ${e}`;
  }
}

function baseName(): string {
  return (audioFileName.value || "transcription").replace(/\.[^.]+$/, "");
}
</script>

<template>
  <div class="app">
    <header class="topbar">
      <div class="brand">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
          <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
          <line x1="12" x2="12" y1="19" y2="22" />
        </svg>
        <span>Расшифровка аудио</span>
      </div>
      <button class="ghost-btn" @click="showSettings = !showSettings">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z" />
        </svg>
        Настройки
      </button>
    </header>

    <div class="layout" :class="{ 'with-panel': showSettings }">
      <!-- ===================== Панель настроек ===================== -->
      <aside v-show="showSettings" class="panel">
        <section class="panel-block">
          <h3>Модель распознавания</h3>
          <div class="model-list">
            <div
              v-for="m in models"
              :key="m.id"
              class="model-row"
              :class="{ active: m.installed && m.id === selectedModelId }"
            >
              <label class="model-pick">
                <input
                  type="radio"
                  name="model"
                  :value="m.id"
                  v-model="selectedModelId"
                  :disabled="!m.installed"
                />
                <span class="model-meta">
                  <span class="model-name">{{ m.name }}</span>
                  <span class="model-desc">{{ m.description }} · {{ m.size_label }}</span>
                </span>
              </label>
              <div class="model-actions">
                <template v-if="downloadingId === m.id">
                  <span class="dl-progress">{{ downloadPercent }}%</span>
                </template>
                <button
                  v-else-if="!m.installed"
                  class="mini-btn"
                  @click="downloadModel(m)"
                  :disabled="!!downloadingId"
                  title="Скачать"
                >
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                    stroke-linecap="round" stroke-linejoin="round">
                    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                    <polyline points="7 10 12 15 17 10" />
                    <line x1="12" y1="15" x2="12" y2="3" />
                  </svg>
                </button>
                <button
                  v-else
                  class="mini-btn danger"
                  @click="deleteModel(m)"
                  title="Удалить"
                >
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                    stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="3 6 5 6 21 6" />
                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                  </svg>
                </button>
              </div>
              <div v-if="downloadingId === m.id" class="dl-bar">
                <div class="dl-fill" :style="{ width: downloadPercent + '%' }"></div>
              </div>
            </div>
          </div>
        </section>

        <section class="panel-block">
          <h3>Язык аудио</h3>
          <select v-model="selectedLanguage" class="select">
            <option v-for="l in LANGUAGES" :key="l.code" :value="l.code">
              {{ l.label }}
            </option>
          </select>
        </section>

        <section class="panel-block">
          <h3>Шумоподавление</h3>
          <label class="switch-row">
            <input type="checkbox" v-model="denoiseEnabled" />
            <span>Очищать запись от шума (FFmpeg)</span>
          </label>
          <p class="hint">
            Подавление фонового шума, срез гула и нормализация громкости перед
            распознаванием. Если FFmpeg не установлен — шаг будет пропущен.
          </p>
        </section>
      </aside>

      <!-- ===================== Рабочая область ===================== -->
      <main class="work">
        <div v-if="!hasInstalledModel" class="empty-state">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
            <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
            <line x1="12" y1="22.08" x2="12" y2="12" />
          </svg>
          <h2>Установите модель</h2>
          <p>Откройте «Настройки» и скачайте хотя бы одну модель Whisper, чтобы начать.</p>
        </div>

        <template v-else>
          <div v-if="!audioFilePath" class="drop-zone" @click="openFileDialog">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"
              stroke-linecap="round" stroke-linejoin="round">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="17 8 12 3 7 8" />
              <line x1="12" x2="12" y1="3" y2="15" />
            </svg>
            <h3>Выберите аудиофайл</h3>
            <p>MP3, WAV, OGG, M4A, FLAC, AAC, WEBM</p>
          </div>

          <div v-else class="file-block">
            <div class="file-card">
              <div class="file-icon">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                  stroke-linecap="round" stroke-linejoin="round">
                  <path d="M9 18V5l12-2v13" />
                  <circle cx="6" cy="18" r="3" />
                  <circle cx="18" cy="16" r="3" />
                </svg>
              </div>
              <div class="file-details">
                <h3>{{ audioFileName }}</h3>
                <p>
                  Модель: {{ activeModel?.name }} ·
                  Язык: {{ LANGUAGES.find((l) => l.code === selectedLanguage)?.label }}
                  <template v-if="denoiseEnabled"> · Шумоподавление</template>
                </p>
              </div>
              <button class="btn-icon" @click="removeFile" title="Убрать файл">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                  stroke-linecap="round" stroke-linejoin="round">
                  <line x1="18" x2="6" y1="6" y2="18" />
                  <line x1="6" x2="18" y1="6" y2="18" />
                </svg>
              </button>
            </div>

            <button class="btn-primary" @click="transcribeAudio" :disabled="isProcessing">
              <span v-if="!isProcessing">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                  stroke-linecap="round" stroke-linejoin="round">
                  <polygon points="5 3 19 12 5 21 5 3" />
                </svg>
                Начать расшифровку
              </span>
              <span v-else class="processing">
                <span class="spinner"></span>
                {{ stageLabel }} {{ transcribePercent }}%
              </span>
            </button>

            <div v-if="isProcessing" class="progress-bar">
              <div class="progress-fill" :style="{ width: transcribePercent + '%' }"></div>
            </div>
          </div>

          <div v-if="error" class="alert alert-error">{{ error }}</div>
          <div v-if="notice" class="alert alert-notice">{{ notice }}</div>

          <div v-if="transcription" class="result">
            <div class="result-head">
              <h2>Результат</h2>
              <div class="result-actions">
                <button class="btn-secondary" @click="copyTranscription">Копировать</button>
                <button class="btn-secondary" @click="exportAs('txt')">.txt</button>
                <button class="btn-secondary" @click="exportAs('srt')">.srt</button>
                <button class="btn-secondary" @click="exportAs('md')">.md</button>
              </div>
            </div>
            <div class="result-text">{{ transcription }}</div>
          </div>
        </template>
      </main>
    </div>
  </div>
</template>

<style scoped>
.app {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--bg);
  color: var(--fg);
  position: relative;
}
/* мягкое цветное свечение на фоне */
.app::before {
  content: "";
  position: fixed;
  inset: 0;
  background:
    radial-gradient(60rem 40rem at 12% -10%, var(--glow-1), transparent 60%),
    radial-gradient(50rem 40rem at 110% 0%, var(--glow-2), transparent 55%);
  pointer-events: none;
  z-index: 0;
}
.app > * { position: relative; z-index: 1; }

.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.9rem 1.5rem;
  border-bottom: 1px solid var(--border);
  background: var(--surface-blur);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
}
.brand {
  display: flex;
  align-items: center;
  gap: 0.7rem;
  font-weight: 700;
  font-size: 1.12rem;
  letter-spacing: -0.01em;
}
.brand svg {
  width: 22px; height: 22px; color: #fff;
  padding: 8px;
  box-sizing: content-box;
  background: linear-gradient(135deg, var(--accent) 0%, var(--accent-2) 100%);
  border-radius: 12px;
  box-shadow: 0 4px 12px var(--accent-shadow);
}

.ghost-btn {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.55rem 0.95rem;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--fg);
  border-radius: 12px;
  font-size: 0.9rem;
  font-weight: 600;
  cursor: pointer;
  transition: 0.2s;
}
.ghost-btn:hover { border-color: var(--accent); color: var(--accent); background: var(--accent-soft); }
.ghost-btn svg { width: 16px; height: 16px; }

.layout {
  flex: 1;
  display: grid;
  grid-template-columns: 1fr;
  overflow: hidden;
}
.layout.with-panel { grid-template-columns: 320px 1fr; }

.panel {
  border-right: 1px solid var(--border);
  background: var(--surface-blur);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  overflow-y: auto;
  padding: 1.4rem;
  display: flex;
  flex-direction: column;
  gap: 1.6rem;
}
.panel-block h3 {
  margin: 0 0 0.8rem;
  font-size: 0.72rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--muted);
}

.model-list { display: flex; flex-direction: column; gap: 0.5rem; }
.model-row {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.7rem 0.8rem;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--surface);
  transition: 0.18s;
}
.model-row:hover { border-color: var(--accent-30); }
.model-row.active {
  border-color: var(--accent);
  background: var(--accent-soft);
  box-shadow: 0 0 0 3px var(--accent-ring);
}
.model-pick { display: flex; align-items: center; gap: 0.6rem; cursor: pointer; flex: 1; }
.model-pick input { accent-color: var(--accent); }
.model-meta { display: flex; flex-direction: column; }
.model-name { font-weight: 600; font-size: 0.92rem; }
.model-desc { font-size: 0.75rem; color: var(--muted); }
.model-actions { flex-shrink: 0; }

.mini-btn {
  width: 34px; height: 34px;
  display: flex; align-items: center; justify-content: center;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--accent);
  border-radius: 10px; cursor: pointer; transition: 0.2s;
}
.mini-btn:hover:not(:disabled) { background: var(--accent-soft); }
.mini-btn:disabled { opacity: 0.4; cursor: not-allowed; }
.mini-btn.danger { color: #ff453a; }
.mini-btn.danger:hover { background: rgba(255, 69, 58, 0.12); }
.mini-btn svg { width: 16px; height: 16px; }

.dl-progress { font-size: 0.8rem; font-weight: 700; color: var(--accent); }
.dl-bar {
  position: absolute; left: 0.7rem; right: 0.7rem; bottom: 4px;
  height: 3px; background: var(--border); border-radius: 2px; overflow: hidden;
}
.dl-fill { height: 100%; background: var(--accent); transition: width 0.2s; }

.select {
  width: 100%;
  padding: 0.7rem 0.8rem;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface);
  color: var(--fg);
  font-size: 0.92rem;
  cursor: pointer;
  transition: 0.18s;
}
.select:hover { border-color: var(--accent-30); }
.select:focus { outline: none; border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-ring); }

.switch-row {
  display: flex; align-items: center; gap: 0.6rem;
  font-size: 0.92rem; cursor: pointer;
}
.switch-row input { width: 18px; height: 18px; accent-color: var(--accent); }
.hint { margin: 0.5rem 0 0; font-size: 0.78rem; color: var(--muted); line-height: 1.5; }

.work {
  overflow-y: auto;
  padding: 2.5rem;
  display: flex;
  flex-direction: column;
  gap: 1.4rem;
  max-width: 920px;
  width: 100%;
  margin: 0 auto;
}

.empty-state, .drop-zone {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  text-align: center; gap: 0.4rem;
  border-radius: 18px;
}
.empty-state { color: var(--muted); padding: 3rem; }
.empty-state svg, .drop-zone svg { width: 56px; height: 56px; color: var(--accent); margin-bottom: 0.5rem; }
.empty-state h2 { margin: 0; color: var(--fg); }

.drop-zone {
  border: 2px dashed var(--border);
  padding: 4rem 2rem;
  cursor: pointer;
  transition: 0.25s;
  background: var(--surface);
  border-radius: 22px;
}
.drop-zone:hover {
  border-color: var(--accent);
  background: var(--accent-soft);
  transform: translateY(-3px);
  box-shadow: 0 16px 40px -16px var(--accent-shadow);
}
.drop-zone h3 { margin: 0.5rem 0 0; }
.drop-zone p { margin: 0; color: var(--muted); font-size: 0.88rem; }

.file-block { display: flex; flex-direction: column; gap: 1rem; }
.file-card {
  display: flex; align-items: center; gap: 1rem;
  padding: 1.1rem 1.35rem;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 18px;
  box-shadow: var(--shadow-sm);
}
.file-icon {
  flex-shrink: 0; width: 52px; height: 52px;
  display: flex; align-items: center; justify-content: center;
  background: var(--accent-soft); color: var(--accent); border-radius: 12px;
}
.file-icon svg { width: 28px; height: 28px; }
.file-details { flex: 1; min-width: 0; }
.file-details h3 { margin: 0 0 0.2rem; font-size: 1rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.file-details p { margin: 0; color: var(--muted); font-size: 0.82rem; }

.btn-icon {
  flex-shrink: 0; width: 38px; height: 38px;
  display: flex; align-items: center; justify-content: center;
  border: none; background: rgba(255, 69, 58, 0.1); color: #ff453a;
  border-radius: 10px; cursor: pointer; transition: 0.2s;
}
.btn-icon:hover { background: rgba(255, 69, 58, 0.2); }
.btn-icon svg { width: 18px; height: 18px; }

.btn-primary {
  width: 100%;
  padding: 1.05rem;
  font-size: 1.05rem;
  font-weight: 650;
  letter-spacing: -0.01em;
  color: #fff;
  background: linear-gradient(135deg, var(--accent) 0%, var(--accent-2) 100%);
  border: none; border-radius: 16px; cursor: pointer; transition: 0.25s;
  box-shadow: 0 8px 24px -6px var(--accent-shadow);
}
.btn-primary:hover:not(:disabled) { transform: translateY(-2px); box-shadow: 0 12px 30px -6px var(--accent-shadow); filter: brightness(1.05); }
.btn-primary:active:not(:disabled) { transform: translateY(0); }
.btn-primary:disabled { opacity: 0.7; cursor: not-allowed; }
.btn-primary span { display: inline-flex; align-items: center; gap: 0.5rem; justify-content: center; }
.btn-primary svg { width: 20px; height: 20px; }

.progress-bar {
  height: 6px;
  background: var(--border);
  border-radius: 3px;
  overflow: hidden;
}
.progress-fill {
  height: 100%;
  background: linear-gradient(135deg, var(--accent) 0%, var(--accent-2) 100%);
  transition: width 0.25s ease;
}

.processing { display: inline-flex; align-items: center; gap: 0.6rem; }
.spinner {
  width: 18px; height: 18px;
  border: 3px solid rgba(255, 255, 255, 0.3);
  border-top-color: #fff; border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.alert { padding: 0.85rem 1.1rem; border-radius: 12px; font-size: 0.9rem; }
.alert-error { background: rgba(255, 69, 58, 0.1); color: #ff453a; border: 1px solid rgba(255, 69, 58, 0.2); }
.alert-notice { background: rgba(255, 159, 10, 0.12); color: #c77700; border: 1px solid rgba(255, 159, 10, 0.25); }

.result { display: flex; flex-direction: column; gap: 0.75rem; }
.result-head { display: flex; align-items: center; justify-content: space-between; }
.result-head h2 { margin: 0; font-size: 1.3rem; }
.result-actions { display: flex; gap: 0.5rem; }
.btn-secondary {
  padding: 0.58rem 1.05rem;
  font-size: 0.88rem; font-weight: 600;
  color: var(--accent); background: var(--accent-soft);
  border: 1px solid var(--accent-30); border-radius: 12px; cursor: pointer; transition: 0.2s;
}
.btn-secondary:hover { background: var(--accent); color: #fff; border-color: var(--accent); transform: translateY(-1px); }
.result-text {
  background: var(--surface);
  border: 1px solid var(--border);
  padding: 1.5rem;
  border-radius: 18px;
  line-height: 1.8;
  white-space: pre-wrap;
  word-wrap: break-word;
  max-height: 50vh;
  overflow-y: auto;
  box-shadow: var(--shadow-sm);
}

@media (max-width: 760px) {
  .layout.with-panel { grid-template-columns: 1fr; }
  .panel { border-right: none; border-bottom: 1px solid var(--border); }
}
</style>

<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
html, body, #app { height: 100vh; width: 100vw; overflow: hidden; }

:root {
  --bg: #f6f7fb;
  --surface: #ffffff;
  --surface-blur: rgba(255, 255, 255, 0.72);
  --fg: #0f1222;
  --muted: #6b7185;
  --border: #e7e9f0;
  --accent: #6d5efc;
  --accent-2: #a855f7;
  --accent-soft: rgba(109, 94, 252, 0.09);
  --accent-30: rgba(109, 94, 252, 0.32);
  --accent-ring: rgba(109, 94, 252, 0.18);
  --accent-shadow: rgba(109, 94, 252, 0.45);
  --glow-1: rgba(109, 94, 252, 0.14);
  --glow-2: rgba(168, 85, 247, 0.12);
  --shadow-sm: 0 2px 10px -4px rgba(15, 18, 34, 0.12);

  font-family: "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 16px;
  -webkit-font-smoothing: antialiased;
  letter-spacing: -0.005em;
}

@media (prefers-color-scheme: dark) {
  :root {
    --bg: #0c0d14;
    --surface: #16171f;
    --surface-blur: rgba(22, 23, 31, 0.72);
    --fg: #eef0f7;
    --muted: #9296a8;
    --border: #262834;
    --accent: #8b7dff;
    --accent-2: #c084fc;
    --accent-soft: rgba(139, 125, 255, 0.13);
    --accent-30: rgba(139, 125, 255, 0.34);
    --accent-ring: rgba(139, 125, 255, 0.22);
    --accent-shadow: rgba(139, 125, 255, 0.5);
    --glow-1: rgba(139, 125, 255, 0.16);
    --glow-2: rgba(192, 132, 252, 0.13);
    --shadow-sm: 0 2px 12px -4px rgba(0, 0, 0, 0.5);
  }
}
</style>
