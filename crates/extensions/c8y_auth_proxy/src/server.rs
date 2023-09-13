use crate::tokens::*;
use axum::body::Body;
use axum::body::Full;
use axum::body::StreamBody;
use axum::extract::FromRef;
use axum::extract::State;
use axum::http::uri::PathAndQuery;
use axum::http::HeaderValue;
use axum::response::Response;
use axum::routing::get;
use axum::routing::IntoMakeService;
use axum::Router;
use futures::future::BoxFuture;
use futures::FutureExt;
use hyper::server::conn::AddrIncoming;
use hyper::HeaderMap;
use hyper::Uri;
use miette::Context;
use miette::IntoDiagnostic;
use reqwest::Method;
use reqwest::StatusCode;
use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::info;

type AxumServer = hyper::Server<AddrIncoming, IntoMakeService<Router>>;

pub struct Server {
    fut: BoxFuture<'static, hyper::Result<()>>,
}

impl Server {
    pub(crate) fn try_init(state: AppState, address: IpAddr, port: u16) -> miette::Result<Self> {
        Ok(Server {
            fut: try_run_server(address, port, state)?.boxed(),
        })
    }

    pub fn wait(self) -> BoxFuture<'static, hyper::Result<()>> {
        self.fut
    }
}

fn try_run_server(address: IpAddr, port: u16, state: AppState) -> miette::Result<AxumServer> {
    info!("Launching on port {port}");
    let handle = get(respond_to)
        .post(respond_to)
        .put(respond_to)
        .patch(respond_to)
        .delete(respond_to)
        .options(respond_to);
    let app = Router::new()
        .route("/", handle.clone())
        .route("/*path", handle)
        .with_state(state);
    Ok(axum::Server::try_bind(&(address, port).into())
        .into_diagnostic()
        .wrap_err_with(|| format!("binding to port {port}"))?
        .serve(app.into_make_service()))
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub target_host: Arc<str>,
    pub token_manager: SharedTokenManager,
}

impl FromRef<AppState> for TargetHost {
    fn from_ref(input: &AppState) -> Self {
        Self(input.target_host.clone())
    }
}

impl FromRef<AppState> for SharedTokenManager {
    fn from_ref(input: &AppState) -> Self {
        input.token_manager.clone()
    }
}

#[derive(Clone)]
struct TargetHost(Arc<str>);

impl fmt::Display for TargetHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

async fn respond_to(
    State(TargetHost(host)): State<TargetHost>,
    retrieve_token: State<SharedTokenManager>,
    uri: Uri,
    method: Method,
    headers: HeaderMap<HeaderValue>,
    small_body: crate::body::PossiblySmallBody,
) -> Response {
    if uri.path().ends_with(".js") || uri.path().starts_with("apps/") {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(<_>::default())
            .unwrap();
    }
    let destination = format!(
        "{host}{}",
        uri.path_and_query()
            .map_or(PathAndQuery::from_static(""), <_>::to_owned)
    );

    let mut token = retrieve_token.not_matching(None).await;

    let client = reqwest::Client::new();
    let (body, body_clone) = small_body.try_clone();
    let send_request = |body, token| {
        client
            .request(method.to_owned(), destination.clone())
            .headers(headers.clone())
            .bearer_auth(&token)
            .body(body)
            .send()
    };
    let mut res = send_request(body, token.clone()).await.unwrap();

    if res.status() == StatusCode::UNAUTHORIZED {
        token = retrieve_token.not_matching(Some(&token)).await;
        if let Some(body) = body_clone {
            res = send_request(Body::from(body), token.clone()).await.unwrap();
        }
    }
    let te_header = res.headers_mut().remove("transfer-encoding");
    let headers = std::mem::take(res.headers_mut());
    let mut build = Response::builder().status(res.status());
    *build.headers_mut().unwrap() = headers;

    let body = if te_header.map_or(false, |h| {
        h.to_str().unwrap_or_default().contains("chunked")
    }) {
        axum::body::boxed(StreamBody::new(res.bytes_stream()))
    } else {
        axum::body::boxed(Full::new(res.bytes().await.unwrap()))
    };

    build.body(body).unwrap()
}

#[cfg(test)]
mod tests {
    use axum::async_trait;
    use axum::body::Bytes;
    use c8y_http_proxy::credentials::JwtRequest;
    use c8y_http_proxy::credentials::JwtResult;
    use c8y_http_proxy::credentials::JwtRetriever;
    use std::borrow::Cow;
    use std::net::Ipv4Addr;
    use tedge_actors::Sequential;
    use tedge_actors::Server;
    use tedge_actors::ServerActorBuilder;
    use tedge_actors::ServerConfig;

    use super::*;

    #[tokio::test]
    async fn forwards_successful_responses() {
        let _ = env_logger::try_init();
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/hello")
            .match_header("Authorization", "Bearer test-token")
            .with_status(204)
            .create();

        let port = start_server(&server, vec!["test-token"]);

        let res = reqwest::get(format!("http://localhost:{port}/hello"))
            .await
            .unwrap();
        assert_eq!(res.status(), 204);
    }

    #[tokio::test]
    async fn forwards_unsuccessful_responses() {
        let _ = env_logger::try_init();
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/not-a-known-url")
            .with_status(404)
            .create();

        let port = start_server(&server, vec!["test-token"]);

        let res = reqwest::get(format!("http://localhost:{port}/not-a-known-url"))
            .await
            .unwrap();
        assert_eq!(res.status(), 404);
    }

    #[tokio::test]
    async fn retries_requests_with_small_bodies() {
        let _ = env_logger::try_init();
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("PUT", "/hello")
            .match_header("Authorization", "Bearer old-token")
            .with_status(401)
            .create();
        let _mock = server
            .mock("PUT", "/hello")
            .match_header("Authorization", "Bearer test-token")
            .match_body("A body")
            .with_body("Some response")
            .with_status(200)
            .create();

        let port = start_server(&server, vec!["old-token", "test-token"]);

        let client = reqwest::Client::new();
        let body = "A body";
        let res = client
            .put(format!("http://localhost:{port}/hello"))
            .header("Content-Length", body.bytes().len())
            .body(body)
            .send()
            .await
            .unwrap();

        assert_eq!(res.status(), 200);
        assert_eq!(res.bytes().await.unwrap(), Bytes::from("Some response"));
    }

    #[tokio::test]
    async fn retries_get_request_on_401() {
        let _ = env_logger::try_init();
        let mut server = mockito::Server::new();
        server
            .mock("GET", "/hello")
            .match_header("Authorization", "Bearer stale-token")
            .with_status(401)
            .with_body("Token expired")
            .create();
        server
            .mock("GET", "/hello")
            .match_header("Authorization", "Bearer test-token")
            .with_status(200)
            .with_body("Succeeded")
            .create();

        let port = start_server(&server, vec!["stale-token", "test-token"]);

        let res = reqwest::get(format!("http://localhost:{port}/hello"))
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        assert_eq!(res.bytes().await.unwrap(), Bytes::from("Succeeded"));
    }

    fn start_server(server: &mockito::Server, tokens: Vec<impl Into<Cow<'static, str>>>) -> u16 {
        let mut retriever = IterJwtRetriever::builder(tokens);
        for port in 3000..3100 {
            let state = AppState {
                target_host: server.url().into(),
                token_manager: TokenManager::new(JwtRetriever::new("TEST => JWT", &mut retriever))
                    .shared(),
            };
            if let Ok(server) = try_run_server(Ipv4Addr::LOCALHOST.into(), port, state) {
                tokio::spawn(server);
                tokio::spawn(retriever.run());
                return port;
            }
        }

        panic!("Failed to bind to any port");
    }

    /// A JwtRetriever that returns a sequence of JWT tokens
    pub(crate) struct IterJwtRetriever {
        pub tokens: std::vec::IntoIter<Cow<'static, str>>,
    }

    #[async_trait]
    impl Server for IterJwtRetriever {
        type Request = JwtRequest;
        type Response = JwtResult;

        fn name(&self) -> &str {
            "IterJwtRetriever"
        }

        async fn handle(&mut self, _request: Self::Request) -> Self::Response {
            Ok(self.tokens.next().unwrap().into())
        }
    }

    impl IterJwtRetriever {
        pub fn builder(
            tokens: Vec<impl Into<Cow<'static, str>>>,
        ) -> ServerActorBuilder<IterJwtRetriever, Sequential> {
            let server = IterJwtRetriever {
                tokens: tokens
                    .into_iter()
                    .map(|token| token.into())
                    .collect::<Vec<_>>()
                    .into_iter(),
            };
            ServerActorBuilder::new(server, &ServerConfig::default(), Sequential)
        }
    }
}
