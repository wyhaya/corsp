use clap::Parser;
use hyper::client::{Client, HttpConnector};
use hyper::header::{HeaderValue, HOST};
use hyper::service::make_service_fn;
use hyper::{Body, Request, Response, Server, StatusCode, Uri};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use logs::*;
use once_cell::sync::Lazy;
use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

// HTTP client
static CLIENT: Lazy<Client<HttpsConnector<HttpConnector>>> = Lazy::new(|| {
    let https = HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .build();
    Client::builder().build::<_, Body>(https)
});

/// A simple CORS proxy tool
#[derive(Parser)]
struct Args {
    /// Bind port
    #[clap(short, long, default_value_t = 1080)]
    port: u16,
}

#[tokio::main]
async fn main() {
    Logs::new()
        .color(true)
        .target(env!("CARGO_PKG_NAME"))
        .init();

    let args = Args::parse();

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, args.port));

    info!("Serving address: {}", &addr);

    let cors = CorsLayer::very_permissive().allow_credentials(true);

    let service = ServiceBuilder::new().layer(cors).service_fn(proxy);

    let make_service = make_service_fn(move |_| {
        let service = service.clone();
        async { Ok::<_, Infallible>(service) }
    });

    let server = Server::try_bind(&addr)
        .unwrap_or_else(|err| {
            error!("{:?}", err);
            std::process::exit(1)
        })
        .serve(make_service);

    if let Err(e) = server.await {
        error!("Server error: {}", e);
    }
}

fn error_response() -> Response<Body> {
    let mut res = Response::new(Body::from("Error"));
    *res.status_mut() = StatusCode::BAD_GATEWAY;
    res
}

async fn proxy(mut req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let remote = req
        .uri()
        .path_and_query()
        .map(|val| val.as_str())
        .unwrap_or_default();

    // Proxy address

    let rst = remote.trim_start_matches('/').parse::<Uri>();
    let uri = match rst {
        Ok(u) => u,
        Err(_) => return Ok(error_response()),
    };

    *req.uri_mut() = uri.clone();

    // Set 'host' header

    let rst = req
        .uri()
        .authority()
        .map(|a| a.as_str())
        .unwrap_or_default()
        .parse::<HeaderValue>();
    let host = match rst {
        Ok(h) => h,
        Err(_) => return Ok(error_response()),
    };

    req.headers_mut().insert(HOST, host);

    info!("Request: {}", &uri);

    let res = CLIENT.request(req).await.unwrap_or_else(|err| {
        error!("{} '{}'", uri, err);
        error_response()
    });

    info!("Request: {} [{}]", &uri, res.status());

    Ok(res)
}
