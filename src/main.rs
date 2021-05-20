use kvarn::prelude::*;
use kvarn_extensions::*;

macro_rules! return_status {
    ($result:expr, $status:expr, $host:expr) => {
        match $result {
            Ok(v) => v,
            Err(_) => {
                return default_error_response($status, $host, None).await;
            }
        }
    };
}

#[tokio::main]
async fn main() {
    let env_log = env_logger::Env::default().default_filter_or("rustls=off,warn");
    env_logger::Builder::from_env(env_log).init();

    let mut args = std::env::args().peekable();

    let executable = args.next().unwrap_or_default();

    if args.peek().map(String::as_str) == Some("--help") {
        println!("Usage: {} [--help] data-path port", executable);
        std::process::exit(0);
    }

    let data_path = args.next().unwrap_or_else(|| {
        let path = String::from("/home/onlinecode");
        warn!("No data path specified. Using '{}'", path);
        path
    });
    let data_path = Arc::new(PathBuf::from(data_path));
    let web_path = data_path.join("web");
    let port: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or_else(|| {
        let port = 8080;
        warn!("No port specified. Using '{}'", port);
        port
    });

    // Mount all extensions to server
    let mut extensions = kvarn::Extensions::empty();

    let proxy = ReverseProxy::base(
        "/code",
        static_connection(ReverseProxyConnection::Tcp(localhost(port))),
    );
    proxy.mount(&mut extensions);

    // let index_data_path = Arc::clone(&data_path);
    extensions.add_prime(prime!(req, _host, _addr {
        if req.uri().path() == "/"{
            let mut uri = req.uri().clone().into_parts();
            uri.path_and_query = Some(uri::PathAndQuery::from_static("/index.html"));
            let uri = Uri::from_parts(uri).unwrap();
            return Some(uri)
        }
        None
    }));

    extensions.add_prepare_fn(
        Box::new(|req| req.uri().path().starts_with("/files/")),
        prepare!(req, host, _path, _addr, move |data_path| {
            let path = match req.uri().path().strip_prefix("/files/") {
                Some(s) => s,
                None => {
                    return utility::default_error_response(
                        StatusCode::BAD_REQUEST,
                        host,
                        Some("Path didn't start with /files/"),
                    )
                    .await
                }
            };

            if path.starts_with('.') {
                return FatResponse::no_cache(Response::new(Bytes::from_static(
                    b"Please do not start the path with a dot.",
                )));
            }

            let base_path = data_path.join(path);
            if base_path
                .as_os_str()
                .to_str()
                .map_or(false, |s| s.ends_with('/'))
            {
                let entries =
                    return_status!(std::fs::read_dir(&base_path), StatusCode::NOT_FOUND, host);
                let dir_header = &b"<h1>Directory contents</h1>"[..];
                let mut bytes = entries.fold(BytesMut::from(dir_header), |mut bytes, entry| {
                    if let Ok(entry) = entry {
                        let absolute_path = entry.path();
                        if let Some(mut path) = absolute_path.to_str() {
                            if let Ok(relative) = absolute_path.strip_prefix(&base_path) {
                                if let Some(relative) = relative.to_str() {
                                    path = relative;
                                }
                            }
                            let entry_str =
                                format!("<p><a href='{path}'>{path}</a></p>", path = path);
                            bytes.extend(entry_str.as_bytes());
                        }
                    }
                    bytes
                });

                if bytes == dir_header {
                    bytes.extend(b"<p>Directory is empty.</p>");
                }
                let bytes = bytes.freeze();
                return FatResponse::no_cache(Response::new(bytes));
            }

            info!("Reading {:?}", base_path);

            let file = utility::read_file(&base_path, &host.file_cache).await;

            let file = match file {
                Some(f) => f,
                None => {
                    return utility::default_error_response(StatusCode::NOT_FOUND, host, None).await
                }
            };

            let response = Response::new(file);

            FatResponse::no_cache(response)
        }),
    );

    {
        let servers_dir = "/server/";
        let path = Arc::new(servers_dir.to_owned());

        let get_port = |req: &Request<_>, path: &str| {
            let req_path = req.uri().path();
            req_path
                .strip_prefix(path)
                .map(|path| {
                    let mut i = path.split('/');
                    (i.next(), i.next())
                })
                .and_then(|(first, second)| {
                    second?;
                    first
                })
                .and_then(|port| port.parse::<u16>().ok())
        };

        let when_path = Arc::clone(&path);
        let when = Box::new(move |request: &FatRequest| get_port(request, &*when_path).is_some());

        let con_path = Arc::clone(&path);
        let connection: reverse_proxy::GetConnectionFn = Arc::new(move |request, _bytes| {
            get_port(request, &*con_path)
                .map(localhost)
                .map(ReverseProxyConnection::Tcp)
        });

        let modify: reverse_proxy::ModifyRequestFn = Arc::new(move |request, _| {
            let path = Arc::clone(&path);

            // We know this is a good path and query; we've just removed the first x bytes.
            let stripped_path = request.uri().path().get(path.as_str().len()..);
            if let Some(stripped_path) = stripped_path {
                let pos = stripped_path.find('/').map(|pos| pos + path.len());
                if let Some(pos) = pos {
                    let mut parts = request.uri().clone().into_parts();

                    if let Some(short_path) = request.uri().path().get(pos..) {
                        let short = uri::PathAndQuery::from_maybe_shared(Bytes::copy_from_slice(
                            short_path.as_bytes(),
                        ))
                        .unwrap();
                        parts.path_and_query = Some(short);
                        parts.scheme = Some(uri::Scheme::HTTP);
                        // For unwrap, see ↑
                        let uri = Uri::from_parts(parts).unwrap();
                        *request.uri_mut() = uri;
                    }
                }
            }
        });
        ReverseProxy::new(when, connection, modify).mount(&mut extensions);
    }

    let host = Host::new("icelk.dev", "cert.pem", "pk.pem", web_path, extensions)
    .expect("failed to construct host. Make sure the certificate and private key are in the current directory");
    // let host = Host::non_secure("icelk.dev", web_path, extensions);

    let data = Arc::new(Data::new(host));

    let descriptor = PortDescriptor::new(100, data);

    let shutdown = kvarn::run(vec![descriptor]).await;

    shutdown.wait().await;
}
