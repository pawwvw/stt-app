# Linux/Mac Installation Instructions

## For Linux/Mac users

This application requires `whisper.cpp` to be installed on your system.

### Option 1: Install from package manager

**Ubuntu/Debian:**
```bash
# whisper.cpp is not yet in official repos, use option 2
```

**macOS (Homebrew):**
```bash
brew install whisper-cpp
```

### Option 2: Build from source

```bash
# Clone the repository
git clone https://github.com/ggerganov/whisper.cpp
cd whisper.cpp

# Build
make


After whisper.cpp is installed, you can run the STT app normally.
