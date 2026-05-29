Сюда нужно положить бинарник FFmpeg, чтобы шумоподавление работало "из коробки".

Windows:
  1. Скачайте сборку с https://www.gyan.dev/ffmpeg/builds/ (ffmpeg-release-essentials.zip)
     или с https://github.com/BtbN/FFmpeg-Builds/releases
  2. Из архива возьмите файл bin/ffmpeg.exe
  3. Положите его в эту папку (src-tauri/whisher/ffmpeg.exe)

Linux:
  1. apt download ffmpeg  ИЛИ возьмите статическую сборку с https://johnvansickle.com/ffmpeg/
  2. Положите исполняемый файл сюда как  ffmpeg  (без расширения), chmod +x ffmpeg

macOS:
  1. brew install ffmpeg  →  скопируйте бинарник:  cp $(which ffmpeg) ./ffmpeg
  2. Положите сюда как  ffmpeg

Приложение ищет ffmpeg в таком порядке:
  1) этот бандлёный бинарник (рядом с whisper-cli),
  2) системный ffmpeg в PATH.

Если ffmpeg не найден нигде — расшифровка всё равно работает, просто без шумоподавления.
