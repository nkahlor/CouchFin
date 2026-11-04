# docker-rust-hello

A simple Rust server/microservice example for [Docker's Rust Language Guide](https://docs.docker.com/language/rust/).

## Running the Slint UI in the Dev Container

Rebuild the Dev Container so it installs the X11 libraries and lightweight desktop. Run the UI from the integrated terminal:

```sh
cargo run
```

Open the **Ports** view in VS Code, select the **Slint desktop** port (6080), and open it in a browser. Connect to the desktop with the default password `vscode`.