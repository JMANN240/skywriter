# SkyWriter

## A Rust-based File Syncing Program

Delete OneDrive, SkyWriter won't spy on you *and* it's written in Rust!

---

Create mappings between files or directories on multiple clients and a server through a simple TOML config file.

### `Config.toml`
```toml
[client.mappings.files]
"path/to/file/on.client"="/absolute/virtual/path/to/file/on.server"

[client.mappings.dirs]
"path/to/dir/on/client"="/absolute/virtual/path/to/dir/on.server"
```

---

## Installation

### Build the server
`cargo build --bin server`

### Build the client
`cargo build --bin client`

## Execution

Some steps are left to the user on both the client and server side. It is recommended to use a reverse proxy such as `nginx` or `apache` with the server for security and performance reasons. A scheduler such as `cron` should be used in conjunction with the client as well to sync files periodically.
