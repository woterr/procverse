![Rust](https://img.shields.io/badge/Rust-2021-000000?style=for-the-badge&logo=rust&logoColor=white)
![Platform](https://img.shields.io/badge/Platform-Linux-000000?style=for-the-badge&logo=linux&logoColor=white)
![UI](https://img.shields.io/badge/UI-Ratatui%20%2B%20Crossterm-000000?style=for-the-badge)

# Procverse (process-universe)

Procverse (process-universe) is a terminal-based Linux process visualization tool written in Rust. It scans the `/proc` filesystem in real time and renders process hierarchies as an interactive graph using a custom force-directed physics engine inside a Ratatui terminal UI.

# Build

```
git clone https://github.com/woterr/procverse.git
cd procverse
```

# Run

```bash
cargo run --release
```

# Screenshot
<img width="1911" height="1149" alt="image" src="https://github.com/user-attachments/assets/e28d0659-0b6c-4db5-a7e2-2862a8f4f963" />
