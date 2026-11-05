# CutCraft

CutCraft is the multi-track video editing and timeline post-production workbench in the Craft Suite application family.

## Credits and Upstream Attribution

CutCraft is based on the open-source video editor [Concat](https://github.com/storytold/concat) (upstream repository: [storytold/concat](https://github.com/storytold/concat), reference fork: [jub0t/concat](https://github.com/jub0t/concat)).
The underlying Concat components are dual-licensed under the Apache License 2.0 and the MIT License. We thank the original authors and contributors for their foundational timeline architecture.

## Changes and Technical Enhancements

CutCraft introduces substantial improvements and suite-specific extensions over upstream Concat:

1. Interface Cleanup and Telemetry Removal:
   - Completely stripped out third-party community banners, external web tracking, and telemetry calls.
   - Resolved project management bugs, including project deletion crashes and timeline state persistence issues.

2. Direct RecordCraft Pipeline Integration:
   - Built-in handoff receiver: recordings completed in RecordCraft automatically instantiate a new CutCraft project with video and audio tracks populated on the timeline in one click.

3. BYOK Video Command AI Sidebar:
   - Custom video-centric AI assistant sidebar supporting OpenAI, Gemini, DeepSeek, Mistral, Qwen, Ollama, and custom endpoints.
   - Capable of issuing timeline commands, generating cut marks, applying title cards, and setting audio volume curves via chat.

4. Suite Standardization:
   - Integrated top header action bar with dynamic Cloud and Auto-Save Sync (Google Drive, OneDrive, local directories).
   - Fully styled in the suite-wide Aurora theme, Dark mode, and Light mode.

## Building and Installation

### Prerequisites
- Rust 1.80 or later (cargo)
- C++ build tools (for Skia rendering and ONNX runtime components)

### Build Command
```bash
cargo build --release -p concat --manifest-path src/Cargo.toml
```

The compiled executable is located at `src/target/release/concat.exe` (or `target/release/cutcraft.exe`).

## License

CutCraft is dual-licensed under the Apache License 2.0 and the MIT License, preserving all upstream notices. See LICENSE-APACHE and LICENSE-MIT.
