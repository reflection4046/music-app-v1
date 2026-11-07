# Rust Mobile Music Player (Android APK)

A touch-first Mobile Music Player written in modern Rust using:
- **GUI Engine:** `egui` & `eframe` with `android-activity`
- **Audio Decoding:** `rodio` 0.19 with Symphonia (MP3, WAV, FLAC, OGG, M4A)
- **Tag Extraction:** `lofty` 0.19 (ID3v2 tags, embedded album art, duration)
- **Concurrency:** `crossbeam-channel` 0.5 (lock-free UI <-> Audio thread)
- **APK Target:** `aarch64-linux-android` (64-bit ARM Android devices)

---

## ⚡ Option 1: Direct 1-Click APK Download
Download the generated package directly from the top bar of the web applet:
- **File:** `rust-mobile-music-player-v1.0.apk`
- **Installation:** Enable *"Install from unknown sources"* in Android Settings and tap the downloaded `.apk`.

---

## 🚀 Option 2: Build Native APK Locally (cargo-ndk)

### 1. Prerequisites
```bash
# 1. Add Android target
rustup target add aarch64-linux-android

# 2. Install cargo-ndk
cargo install cargo-ndk

# 3. Export Android NDK path
export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/25.2.9519653
```

### 2. Compile Native Shared Library (.so)
```bash
cargo ndk -t aarch64-linux-android -o ./android/app/src/main/jniLibs build --release
```

### 3. Assemble Release APK
```bash
cd android
./gradlew assembleRelease
```

The generated APK will be at:
`android/app/build/outputs/apk/release/app-release.apk`

Install to your phone via USB:
```bash
adb install android/app/build/outputs/apk/release/app-release.apk
```

---

## ☁️ Option 3: Automated Cloud APK via GitHub Actions
We include a pre-configured `.github/workflows/build-apk.yml` workflow:
1. Push this repository to GitHub.
2. Go to **Actions** → **Build Android APK** → **Run workflow**.
3. In under 2 minutes, GitHub's Ubuntu runners compile the NDK library, assemble the APK, and upload it as a downloadable artifact!
