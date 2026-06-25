# Mahflow

**Offline speech-to-text for your Mac — created by [Mahip Kakan](https://github.com/mahip-kakan).**

Mahflow is a privacy-focused desktop app that turns your voice into text. Press a shortcut, speak, and your words are pasted into whatever app you are using. Everything runs locally on your computer — no cloud required.

## What Mahflow does

1. **Press** a keyboard shortcut to start or stop recording
2. **Speak** while the shortcut is active
3. **Release** and Mahflow transcribes your speech with on-device AI
4. **Get** text pasted directly into your active application

## Quick start (use the app)

### Download & install (macOS — recommended)

**Requirements:** a Mac with **Apple Silicon** (M1, M2, M3, or M4). Intel Macs are not supported yet.

1. Go to the **[latest release](https://github.com/mahip-kakan/Mahflow/releases/latest)** and download **`Mahflow_<version>_aarch64.dmg`**.
2. **Double-click** the downloaded `.dmg` to open it.
3. **Drag the Mahflow icon onto the Applications folder** in the window that appears.
4. Eject the disk image, then open **Mahflow** from Applications (or Launchpad/Spotlight).

#### First launch: allow the app through macOS security

Mahflow is not yet notarized by Apple, so macOS shows a warning **the first time only**. This is expected. Use any one of these:

- **Easiest:** In Applications, **right-click** (or Control-click) **Mahflow → Open**, then click **Open** in the dialog.
- **Or:** open  **System Settings → Privacy & Security**, scroll down, and click **Open Anyway** next to the Mahflow message, then launch the app again.
- **If you see _"Mahflow is damaged and can't be opened"_,** open the **Terminal** app and run:

```bash
xattr -dr com.apple.quarantine /Applications/Mahflow.app
```

  then open Mahflow normally.

#### After it opens

1. Grant **Microphone** and **Accessibility** permissions when macOS prompts you (or in **System Settings → Privacy & Security**).
2. The first launch **downloads a speech model**, so stay connected to the internet for that step. Everything runs **offline** afterward.
3. Open **Settings**, pick a keyboard shortcut, then press it, speak, and release.

### Install from source (developers)

See [BUILD.md](BUILD.md) for full setup. On Apple Silicon Mac:

```bash
git clone https://github.com/mahip-kakan/Mahflow.git
cd Mahflow
bun install
bun tauri dev
```

### First launch

1. Grant **Microphone** and **Accessibility** permissions when macOS prompts you
2. Open **Settings** and choose a keyboard shortcut
3. Download a transcription model (Parakeet V3 is a good starting point on CPU)
4. Press your shortcut, speak, and release

## Architecture

Mahflow is a [Tauri](https://tauri.app/) app:

- **Frontend** — React + TypeScript (settings UI)
- **Backend** — Rust (audio, shortcuts, local ML inference)
- **Models** — Whisper and Parakeet run fully offline after download

## CLI

```bash
mahflow --toggle-transcription   # Toggle recording
mahflow --cancel                 # Cancel current operation
mahflow --start-hidden           # Launch without showing the window
mahflow --help                   # Show all flags
```

On macOS with the `.app` bundle:

```bash
/Applications/Mahflow.app/Contents/MacOS/mahflow --toggle-transcription
```

## Project structure

```
Mahflow/
├── src/           # React UI (settings, onboarding)
├── src-tauri/     # Rust engine (audio, transcription, system integration)
├── BUILD.md       # Build instructions
└── package.json   # Frontend dependencies
```

## Acknowledgments

Mahflow is built on open-source work including [Whisper](https://github.com/openai/whisper), [Tauri](https://tauri.app/), and the wider speech-to-text community.

## Author

**Mahip Kakan** — [github.com/mahip-kakan](https://github.com/mahip-kakan)

## License

MIT — see [LICENSE](LICENSE).
