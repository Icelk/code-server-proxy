# code-server-proxy

> A proxy for [code-server](https://github.com/cdr/code-server) composing a good online programming experience.
> This is designed to be used by multiple users, concurrently.

A reverse-proxy built on Kvarn to enable TLS on code-server, with extra bits to view your files in the browser (rendered HTML)
and access backend servers by a port.

This means you can program both backend servers and frontend UI from the same website.

# Requirements

This requires you to have [code-server](https://github.com/cdr/code-server) installed and running as a "isolated" user.

The user `onlinecode` and `code-server` listening on port `8080` is assumed, but you can change these defaults on the command line.
See `cargo run -- --help` for more information about usage.
