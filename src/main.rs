use hyper::client::{Client, HttpConnector};
use hyper::header::{HeaderValue, HOST};
use hyper::service::make_service_fn;
use hyper::{Body, Request, Response, Server, StatusCode, Uri};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use logs::*;
use once_cell::sync::Lazy;
use std::convert::Infallible;
use std::net::{Ipv6Addr, SocketAddr};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

static CLIENT: Lazy<Client<HttpsConnector<HttpConnector>>> = Lazy::new(|| {
    let https = HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .build();
    Client::builder().build::<_, Body>(https)
});

#[tokio::main]
async fn main() {
    LogConfig::enable_all()
        .color(true)
        .date_format("%T")
        .unwrap()
        .apply();

    let args = std::env::args().collect::<Vec<String>>();
    let port = match args.get(1) {
        Some(s) => match s.parse::<u16>() {
            Ok(p) => p,
            Err(_) => {
                error!("Invalid port: {}", s);
                std::process::exit(1);
            }
        },
        None => 1080,
    };

    let addr = SocketAddr::from((Ipv6Addr::UNSPECIFIED, port));

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

    let rst = CLIENT.request(req).await.unwrap_or_else(|err| {
        error!("{} '{}'", uri, err);
        error_response()
    });
    Ok(rst)
}
