# Scientific File Viewer

A terminal-based file viewer for scientific data files, built with Rust and [ratatui](https://github.com/ratatui/ratatui).

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

## Features

- 📁 **File Browser** - Navigate your filesystem with an intuitive tree view
- 📊 **Auto Chart Detection** - Automatically detects and visualizes two-column numeric data
- 🎨 **Atom One Dark Theme** - Beautiful, easy-on-the-eyes color scheme
- 🔤 **Nerd Font Support** - Icons with emoji fallback for compatibility
- 📈 **Peak-Preserving Downsampling** - Efficient visualization of large datasets
- 💾 **Session Persistence** - Remembers your last directory
- 📏 **Line Numbers** - Easy reference for file contents

## Supported File Formats

- `.txt`, `.log` - Text files
- `.dat`, `.csv` - Data files (with chart visualization)
- `.xyz`, `.pdb`, `.cif` - Scientific structure files

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/grebenyyk/sci-file-viewer.git
cd sci-file-viewer

# Build in release mode
cargo build --release

# Run the application
./target/release/sci-file-viewer
```

### From Releases

Download the pre-built binary for your platform from the [Releases](https://github.com/grebenyyk/sci-file-viewer/releases) page.

## Usage

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `↑` `↓` | Navigate file tree |
| `Enter` | Open file/directory |
| `Backspace` | Go to parent directory |
| `j` `k` | Scroll content up/down |
| `u` `d` | Page up/down |
| `Home` `End` | Go to start/end of file |
| `~` | Go to home directory |
| `.` | Return to startup directory |
| `c` | Toggle chart panel |
| `n` | Toggle Nerd Fonts/Emoji |
| `r` | Refresh directory |
| `q` | Quit |

### Chart Visualization

The viewer automatically detects files with two-column numeric data and displays a scatter plot. Supported formats:

```
# Comment lines are ignored
1.0  2.5
2.0  4.0
3.0  3.5
```

Data can be separated by spaces, tabs, or commas.

## Screenshots

```
┌─ Files ─────────┬─ Content [1-30/150] ──────────────┬─ Scatter Plot ────────┐
│  📁 data        │  1 │ # Sample data file           │ ⠁⠈⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠁│
│  📁 results     │  2 │ 0.0  1.234                   │ ⠀⠀⠀⠂⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠄⠀⠀⠀⠀│
│▸ 📊 spectrum.dat│  3 │ 0.5  2.456                   │ ⠀⠀⠀⠀⠀⠁⠀⠀⠀⠀⠀⠀⠈⠀⠀⠀⠀⠀⠀│
│  📄 notes.txt   │  4 │ 1.0  3.789                   │ ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀│
│  🔬 molecule.xyz│  5 │ ...                          ├─ Info & Stats ────────┤
└─────────────────┴──────────────────────────────────┤ Type: File            │
│ /home/user/data/spectrum.dat                       │ Size: 1.24 KB         │
│ ↑↓ Nav  Enter Open  Bksp Parent  j/k Scroll  q Quit│ Lines: 150            │
└────────────────────────────────────────────────────┴───────────────────────┘
```

## Building for Other Platforms

The application is cross-platform and can be built on Windows, macOS, and Linux.

### Requirements

- Rust 1.70 or later
- A terminal with UTF-8 support

### Platform-Specific Builds

```bash
# macOS (Intel)
cargo build --release --target x86_64-apple-darwin

# macOS (Apple Silicon)
cargo build --release --target aarch64-apple-darwin

# Linux
cargo build --release --target x86_64-unknown-linux-gnu

# Windows
cargo build --release --target x86_64-pc-windows-msvc
```

## Dependencies

- [ratatui](https://github.com/ratatui/ratatui) - Terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Cross-platform terminal manipulation
- [chrono](https://github.com/chronotope/chrono) - Date and time handling
- [dirs](https://github.com/dirs-dev/dirs-rs) - Platform-specific directories

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request
