# GitOp

A terminal-based git repository monitor with real-time status updates. Monitor multiple git repositories simultaneously in a clean, vim-like interface.

![GitOp Screenshot](screenshot.png)

## Features

- **Real-time monitoring** of multiple git repositories
- **Clean TUI interface** with vim-like navigation
- **Configurable colors** for ahead/behind indicators
- **Repository expansion** to view recent commits
- **Branch tracking** and status display
- **Console output** for commit notifications
- **Cross-platform** support (Linux, Windows, macOS)

## Installation

### Prerequisites
GitOp requires Rust to be installed. If you don't have Rust:

```bash
# Install Rust (one-time setup)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Install GitOp

```bash
# Install latest version from GitLab
cargo install --git https://gitlab.com/yourusername/gitop.git

# Or install a specific version
cargo install --git https://gitlab.com/yourusername/gitop.git --tag v0.1.0
```

### Verify Installation
```bash
# Check it's installed
gitop --help
# or just run
gitop
```

### Update GitOp
```bash
# Update to latest version
cargo install --git https://gitlab.com/yourusername/gitop.git --force
```

### Uninstall
```bash
cargo uninstall gitop
```

## Usage

### Basic Usage

```bash
# Start monitoring current directory
gitop

# The tool will create a default config if none exists
```

### Configuration

Create `gitop.toml` in your project directory or current working directory:

```toml
# GitOp Configuration
refresh_interval = 5  # seconds between git status checks
max_commits = 5       # number of commits to show when expanded

# Color Configuration (Optional)
[colors]
ahead_color = "yellow"   # Color for ↑ arrows
behind_color = "cyan"    # Color for ↓ arrows

# Repository Configuration
[[repositories]]
name = "My Project"
path = "/path/to/your/repo"
remote = "origin"

[[repositories]]
name = "Another Project"
path = "~/projects/another-repo"
remote = "upstream"
```

### Controls

- **↑/↓** - Navigate between repositories
- **Enter** - Expand/collapse repository to show recent commits
- **q** - Quit

### Path Configuration

GitOp supports various path formats:

- **Relative**: `"."` (current directory)
- **Absolute**: `"/home/user/projects/repo"`
- **Tilde expansion**: `"~/projects/repo"`
- **No trailing slash needed**

### Available Colors

- Basic: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `gray`
- Light: `lightred`, `lightgreen`, `lightyellow`, `lightblue`, `lightmagenta`, `lightcyan`
- Dark: `darkgray`
- Special: `reset`, `default`, `normal` (terminal default)
- RGB Hex: `"#FF5500"` or `"FF5500"`

## Display

The interface shows four columns:

1. **Repository** - Repository name
2. **Ahead** - Commits ahead of remote (↑5)
3. **Behind** - Commits behind remote (↓3)
4. **Branch** - Current branch name

When expanded, repositories show recent commits with:
- Commit hash and message
- Author name
- Timestamp (MM/DD HH:MM)
- Branch name

## Console Output

The console at the bottom shows:
- Real-time commit notifications
- Status change alerts
- System messages and errors
- Repository sync notifications

## Examples

### Single Repository
```toml
refresh_interval = 5
max_commits = 3

[[repositories]]
name = "Current Project"
path = "."
remote = "origin"
```

### Multiple Repositories
```toml
refresh_interval = 10
max_commits = 5

[[repositories]]
name = "Work Project"
path = "~/work/important-project"
remote = "origin"

[[repositories]]
name = "Personal Site"
path = "~/projects/website"
remote = "origin"

[[repositories]]
name = "Fork"
path = "~/forks/open-source-project"
remote = "upstream"
```

## Troubleshooting

### "Git error: repository not found"
- Verify the path exists and is a git repository
- Check that the remote exists: `git remote -v`
- Ensure you have permission to access the repository

### "Path does not exist"
- Check the path in your configuration
- Use absolute paths if relative paths aren't working
- Ensure tilde (`~`) expansion is working correctly

### Colors not working
- Verify your terminal supports colors
- Try basic color names instead of RGB hex codes
- Use `"reset"` to use terminal default colors

### "Command not found: gitop"
- Make sure `~/.cargo/bin` is in your PATH
- Restart your terminal after installing Rust
- Check installation: `ls ~/.cargo/bin/gitop`

## Building from Source

If you want to build locally instead of installing:

```bash
# Clone the repository
git clone https://gitlab.com/yourusername/gitop.git
cd gitop

# Build and run
cargo run

# Or build release version
cargo build --release
./target/release/gitop
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a merge request

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Changelog

### v0.1.0
- Initial release
- Real-time git repository monitoring
- Configurable colors and refresh intervals
- Repository expansion with commit history
- Cross-platform support
