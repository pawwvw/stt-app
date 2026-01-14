<script setup lang="ts">
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

const audioFile = ref<File | null>(null);
const audioFilePath = ref<string>("");
const isProcessing = ref(false);
const transcription = ref("");
const error = ref("");

const audioFileName = computed(() => audioFile.value?.name || "");
const audioFileSize = computed(() => {
  if (!audioFile.value) return "";
  const size = audioFile.value.size;
  if (size === 0) return "Аудио файл";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(2)} KB`;
  return `${(size / (1024 * 1024)).toFixed(2)} MB`;
});

async function openFileDialog() {
  try {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Audio",
          extensions: ["mp3", "wav", "ogg", "m4a", "webm", "flac"],
        },
      ],
    });

    if (selected && typeof selected === "string") {
      audioFilePath.value = selected;
      const fileName = selected.split(/[/\\]/).pop() || "audio.mp3";

      audioFile.value = {
        name: fileName,
        size: 0,
      } as File;

      error.value = "";
      transcription.value = "";
    }
  } catch (e) {
    error.value = `Ошибка при выборе файла: ${e}`;
  }
}

function removeFile() {
  audioFile.value = null;
  audioFilePath.value = "";
  transcription.value = "";
  error.value = "";
}

async function transcribeAudio() {
  if (!audioFile.value || !audioFilePath.value) {
    error.value = "Файл не выбран";
    return;
  }

  isProcessing.value = true;
  error.value = "";
  transcription.value = "";

  try {
    const result = await invoke<{
      text: string;
      success: boolean;
      error?: string;
    }>("transcribe_audio", {
      filePath: audioFilePath.value,
    });

    if (result.success) {
      transcription.value = result.text;
      if (result.error) {
        console.warn("Предупреждение:", result.error);
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

function copyTranscription() {
  if (transcription.value) {
    navigator.clipboard.writeText(transcription.value);
  }
}
</script>

<template>
  <main class="container">
    <div class="header">
      <h1>
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="32"
          height="32"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
          <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
          <line x1="12" x2="12" y1="19" y2="22" />
        </svg>
        Расшифровка аудио
      </h1>
      <p class="subtitle">Загрузите аудио файл для преобразования в текст</p>
    </div>

    <div class="upload-section">
      <div v-if="!audioFile" class="drop-zone" @click="openFileDialog">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="64"
          height="64"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="upload-icon"
        >
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <polyline points="17 8 12 3 7 8" />
          <line x1="12" x2="12" y1="3" y2="15" />
        </svg>
        <h3>Выберите аудио файл</h3>
        <p>Нажмите для выбора файла</p>
        <div class="supported-formats">
          <span class="format-badge">MP3</span>
        </div>
      </div>

      <div v-else class="file-info">
        <div class="file-card">
          <div class="file-icon">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="48"
              height="48"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="M9 18V5l12-2v13" />
              <circle cx="6" cy="18" r="3" />
              <circle cx="18" cy="16" r="3" />
            </svg>
          </div>
          <div class="file-details">
            <h3>{{ audioFileName }}</h3>
            <p>{{ audioFileSize }}</p>
          </div>
          <button class="btn-icon" @click="removeFile" title="Удалить файл">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <line x1="18" x2="6" y1="6" y2="18" />
              <line x1="6" x2="18" y1="6" y2="18" />
            </svg>
          </button>
        </div>

        <button
          class="btn-primary"
          @click="transcribeAudio"
          :disabled="isProcessing"
        >
          <span v-if="!isProcessing">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <polygon points="5 3 19 12 5 21 5 3" />
            </svg>
            Начать расшифровку
          </span>
          <span v-else class="processing">
            <div class="spinner"></div>
            Обработка...
          </span>
        </button>
      </div>
    </div>

    <div v-if="error" class="alert alert-error">
      <svg
        xmlns="http://www.w3.org/2000/svg"
        width="20"
        height="20"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <circle cx="12" cy="12" r="10" />
        <line x1="12" x2="12" y1="8" y2="12" />
        <line x1="12" x2="12.01" y1="16" y2="16" />
      </svg>
      {{ error }}
    </div>

    <div v-if="transcription" class="result-section">
      <div class="result-header">
        <h2>Результат расшифровки</h2>
        <button class="btn-secondary" @click="copyTranscription">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <rect width="14" height="14" x="8" y="8" rx="2" ry="2" />
            <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />
          </svg>
          Копировать
        </button>
      </div>
      <div class="transcription-text">
        {{ transcription }}
      </div>
    </div>
  </main>
</template>

<style scoped>
.container {
  max-width: 900px;
  width: 100%;
  height: 100vh;
  margin: 0 auto;
  padding: clamp(1rem, 3vh, 2rem) 1.5rem;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: clamp(1.5rem, 3vh, 2.5rem);
  overflow-y: auto;
  overflow-x: hidden;
}

.header {
  text-align: center;
  margin-bottom: 0;
}

.header h1 {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  font-size: clamp(1.75rem, 5vw, 2.5rem);
  font-weight: 700;
  margin-bottom: 0.5rem;
  color: #1a1a1a;
  flex-wrap: wrap;
}

.header h1 svg {
  color: #646cff;
  width: clamp(24px, 5vw, 32px);
  height: clamp(24px, 5vw, 32px);
}

.subtitle {
  font-size: clamp(0.95rem, 2.5vw, 1.1rem);
  color: #666;
  margin: 0;
  padding: 0 1rem;
}

.upload-section {
  margin-bottom: 0;
}

.drop-zone {
  border: 3px dashed #ccc;
  border-radius: 16px;
  padding: clamp(2rem, 5vw, 3rem) clamp(1rem, 3vw, 2rem);
  text-align: center;
  cursor: pointer;
  transition: all 0.3s ease;
  background: rgba(255, 255, 255, 0.5);
}

.drop-zone:hover {
  border-color: #646cff;
  background: rgba(100, 108, 255, 0.05);
  transform: translateY(-2px);
}

.drop-zone.dragging {
  border-color: #646cff;
  background: rgba(100, 108, 255, 0.1);
  transform: scale(1.02);
}

.upload-icon {
  color: #646cff;
  margin-bottom: 1rem;
  width: clamp(48px, 10vw, 64px);
  height: clamp(48px, 10vw, 64px);
}

.drop-zone h3 {
  margin: 1rem 0 0.5rem;
  font-size: clamp(1.1rem, 3vw, 1.3rem);
  color: #1a1a1a;
}

.drop-zone p {
  color: #666;
  margin-bottom: 1.5rem;
  font-size: clamp(0.9rem, 2vw, 1rem);
}

.supported-formats {
  display: flex;
  gap: 0.5rem;
  justify-content: center;
  flex-wrap: wrap;
}

.format-badge {
  display: inline-block;
  padding: 0.25rem 0.75rem;
  background: rgba(100, 108, 255, 0.1);
  color: #646cff;
  border-radius: 12px;
  font-size: 0.85rem;
  font-weight: 600;
}

.file-info {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.file-card {
  display: flex;
  align-items: center;
  gap: clamp(0.75rem, 3vw, 1.5rem);
  padding: clamp(1rem, 3vw, 1.5rem);
  background: rgba(255, 255, 255, 0.8);
  border-radius: 16px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.file-icon {
  flex-shrink: 0;
  width: clamp(48px, 10vw, 64px);
  height: clamp(48px, 10vw, 64px);
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(100, 108, 255, 0.1);
  border-radius: 12px;
  color: #646cff;
}

.file-icon svg {
  width: clamp(32px, 7vw, 48px);
  height: clamp(32px, 7vw, 48px);
}

.file-details {
  flex: 1;
  min-width: 0;
}

.file-details h3 {
  margin: 0 0 0.25rem;
  font-size: clamp(0.95rem, 2.5vw, 1.1rem);
  color: #1a1a1a;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-details p {
  margin: 0;
  color: #666;
  font-size: clamp(0.85rem, 2vw, 0.9rem);
}

.btn-icon {
  flex-shrink: 0;
  width: 40px;
  height: 40px;
  border: none;
  background: rgba(255, 59, 48, 0.1);
  color: #ff3b30;
  border-radius: 10px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}

.btn-icon:hover {
  background: rgba(255, 59, 48, 0.2);
  transform: scale(1.1);
}

.btn-primary {
  width: 100%;
  padding: clamp(0.85rem, 2.5vw, 1rem) clamp(1.5rem, 4vw, 2rem);
  font-size: clamp(1rem, 2.5vw, 1.1rem);
  font-weight: 600;
  color: white;
  background: linear-gradient(135deg, #646cff 0%, #535bf2 100%);
  border: none;
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.3s ease;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  box-shadow: 0 4px 12px rgba(100, 108, 255, 0.3);
}

.btn-primary:hover:not(:disabled) {
  transform: translateY(-2px);
  box-shadow: 0 6px 20px rgba(100, 108, 255, 0.4);
}

.btn-primary:active:not(:disabled) {
  transform: translateY(0);
}

.btn-primary:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}

.btn-primary span {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.processing {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.spinner {
  width: 20px;
  height: 20px;
  border: 3px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.alert {
  padding: 1rem 1.5rem;
  border-radius: 12px;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 0;
  animation: slideIn 0.3s ease;
}

@keyframes slideIn {
  from {
    opacity: 0;
    transform: translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.alert-error {
  background: rgba(255, 59, 48, 0.1);
  color: #ff3b30;
  border: 1px solid rgba(255, 59, 48, 0.2);
}

.result-section {
  animation: slideIn 0.4s ease;
}

.result-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.75rem;
}

.result-header h2 {
  margin: 0;
  font-size: clamp(1.25rem, 3.5vw, 1.5rem);
  color: #1a1a1a;
}

.btn-secondary {
  padding: clamp(0.5rem, 1.5vw, 0.6rem) clamp(1rem, 3vw, 1.2rem);
  font-size: clamp(0.85rem, 2vw, 0.95rem);
  font-weight: 600;
  color: #646cff;
  background: rgba(100, 108, 255, 0.1);
  border: 1px solid rgba(100, 108, 255, 0.2);
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  white-space: nowrap;
}

.btn-secondary svg {
  width: clamp(16px, 3vw, 18px);
  height: clamp(16px, 3vw, 18px);
}

.btn-secondary:hover {
  background: rgba(100, 108, 255, 0.2);
  transform: translateY(-1px);
}

.transcription-text {
  background: rgba(255, 255, 255, 0.8);
  padding: clamp(1rem, 3vw, 1.5rem);
  border-radius: 12px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  line-height: 1.8;
  color: #1a1a1a;
  font-size: clamp(0.9rem, 2.5vw, 1rem);
  white-space: pre-wrap;
  word-wrap: break-word;
  max-height: 400px;
  overflow-y: auto;
}

/* Scrollbar styling */
.transcription-text::-webkit-scrollbar {
  width: 8px;
}

.transcription-text::-webkit-scrollbar-track {
  background: rgba(0, 0, 0, 0.05);
  border-radius: 4px;
}

.transcription-text::-webkit-scrollbar-thumb {
  background: rgba(100, 108, 255, 0.3);
  border-radius: 4px;
}

.transcription-text::-webkit-scrollbar-thumb:hover {
  background: rgba(100, 108, 255, 0.5);
}

@media (prefers-color-scheme: dark) {
  .header h1 {
    color: #f6f6f6;
  }

  .subtitle {
    color: #aaa;
  }

  .drop-zone {
    background: rgba(255, 255, 255, 0.05);
    border-color: #444;
  }

  .drop-zone:hover {
    background: rgba(100, 108, 255, 0.1);
  }

  .drop-zone h3 {
    color: #f6f6f6;
  }

  .drop-zone p {
    color: #aaa;
  }

  .file-card {
    background: rgba(255, 255, 255, 0.05);
  }

  .file-details h3 {
    color: #f6f6f6;
  }

  .file-details p {
    color: #aaa;
  }

  .result-header h2 {
    color: #f6f6f6;
  }

  .transcription-text {
    background: rgba(255, 255, 255, 0.05);
    color: #f6f6f6;
  }
}

/* Адаптивный дизайн для мобильных устройств */
@media (max-width: 768px) {
  .container {
    padding: 1rem;
    gap: 1.25rem;
  }

  .header {
    margin-bottom: 0;
  }

  .header h1 {
    gap: 0.5rem;
  }

  .drop-zone {
    border-width: 2px;
    padding: 2rem 1rem;
  }

  .supported-formats {
    gap: 0.35rem;
  }

  .format-badge {
    padding: 0.2rem 0.6rem;
    font-size: 0.8rem;
  }

  .file-card {
    gap: 1rem;
    padding: 1rem;
  }

  .btn-icon {
    width: 36px;
    height: 36px;
  }

  .btn-icon svg {
    width: 18px;
    height: 18px;
  }

  .result-header {
    flex-direction: column;
    gap: 1rem;
    align-items: flex-start;
  }

  .btn-secondary {
    width: 100%;
    justify-content: center;
  }

  .alert {
    padding: 0.85rem 1rem;
    font-size: 0.9rem;
  }
}

@media (max-width: 480px) {
  .container {
    padding: 0.75rem;
    gap: 1rem;
  }

  .header {
    margin-bottom: 0;
  }

  .upload-section {
    margin-bottom: 0;
  }

  .drop-zone h3 {
    font-size: 1rem;
  }

  .drop-zone p {
    font-size: 0.85rem;
  }

  .file-icon {
    width: 48px;
    height: 48px;
  }

  .file-icon svg {
    width: 32px;
    height: 32px;
  }

  .file-details h3 {
    font-size: 0.9rem;
  }

  .file-details p {
    font-size: 0.8rem;
  }

  .btn-primary {
    padding: 0.85rem 1.5rem;
  }

  .transcription-text {
    max-height: 300px;
  }
}

/* Адаптация для очень больших экранов */
@media (min-width: 1400px) {
  .container {
    max-width: 1000px;
    padding: 2.5rem 2rem;
    gap: 3rem;
  }
}
</style>
<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html,
body,
#app {
  height: 100vh;
  width: 100vw;
  overflow: hidden;
}

:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;

  color: #0f0f0f;
  background-color: #f6f6f6;

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

.container {
  margin: 0;
  padding-top: 10vh;
  display: flex;
  flex-direction: column;
  justify-content: center;
  text-align: center;
}

.logo {
  height: 6em;
  padding: 1.5em;
  will-change: filter;
  transition: 0.75s;
}

.logo.tauri:hover {
  filter: drop-shadow(0 0 2em #24c8db);
}

.row {
  display: flex;
  justify-content: center;
}

a {
  font-weight: 500;
  color: #646cff;
  text-decoration: inherit;
}

a:hover {
  color: #535bf2;
}

h1 {
  text-align: center;
}

input,
button {
  border-radius: 8px;
  border: 1px solid transparent;
  padding: 0.6em 1.2em;
  font-size: 1em;
  font-weight: 500;
  font-family: inherit;
  color: #0f0f0f;
  background-color: #ffffff;
  transition: border-color 0.25s;
  box-shadow: 0 2px 2px rgba(0, 0, 0, 0.2);
}

button {
  cursor: pointer;
}

button:hover {
  border-color: #396cd8;
}
button:active {
  border-color: #396cd8;
  background-color: #e8e8e8;
}

input,
button {
  outline: none;
}

#greet-input {
  margin-right: 5px;
}

@media (prefers-color-scheme: dark) {
  :root {
    color: #f6f6f6;
    background-color: #2f2f2f;
  }

  a:hover {
    color: #24c8db;
  }

  input,
  button {
    color: #ffffff;
    background-color: #0f0f0f98;
  }
  button:active {
    background-color: #0f0f0f69;
  }
}
</style>
