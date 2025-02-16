use log::{error, info};
use opentelemetry::global;
use opentelemetry::trace::{Span, Tracer};
use opentelemetry::KeyValue;
use serde::Serialize;
use std::env;
use std::sync::Arc;
use warp::{Filter, Rejection, Reply};

use alloy::providers::Provider;
use alloy::providers::ProviderBuilder;
use alloy_primitives::Address;
use url::Url;

#[derive(Serialize)]
struct BalanceResponse {
    balance: String,
}

/// Get the balance for a given Ethereum address.
///
/// # Examples
///
/// ```rust
/// # use std::sync::Arc;
/// # use alloy::provider::Provider;
/// # use alloy::types::Address;
/// # let ethereum_rpc_url = "http://localhost:8545".to_string();
/// # let provider = Provider::try_from(ethereum_rpc_url.as_str()).expect("Failed to create provider");
/// # let provider = Arc::new(provider);
/// # async_std::task::block_on(async {
/// let address = "0x0000000000000000000000000000000000000000".to_string();
/// let response = get_balance(address, provider.clone()).await.unwrap();
/// println!("{:?}", response);
/// # });
/// ```
async fn get_balance(
    address: String,
    provider: Arc<dyn Provider>,
) -> Result<impl Reply, Rejection> {
    // Get the global tracer (avoid passing it around)
    let tracer = global::tracer("example");
    let mut span = tracer.start("get_balance");

    // Parse the address string into an Ethereum Address.
    info!("Parsing address: {}", address);
    let address_parsed = address.parse::<Address>().map_err(|error| {
        error!("Failed to parse address: {}", error);
        warp::reject::custom(ServerError)
    })?;

    // Query the balance via the alloy provider.
    info!("Querying balance for address: {}", address_parsed);
    let balance = provider
        .get_balance(address_parsed)
        .await
        .map_err(|_| warp::reject::custom(ServerError))?;

    info!("Fetched balance: {}", balance);
    span.add_event(
        "Fetched balance",
        vec![KeyValue::new("balance", balance.to_string())],
    );
    span.end();

    Ok(warp::reply::json(&BalanceResponse {
        balance: balance.to_string(),
    }))
}

#[derive(Debug)]
struct ServerError;
impl warp::reject::Reject for ServerError {}

#[tokio::main]
async fn main() {
    env_logger::init();
    info!("Starting the Warp server...");

    let provider = setup_provider().await;

    // Set up CORS and routes
    let cors = setup_cors();
    let routes = setup_routes(provider).with(cors);

    println!("Server starting on http://localhost:3030");
    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}

/// Sets up the Ethereum provider.
///
/// # Examples
///
/// ```rust
/// # async fn test_setup_provider() {
/// let provider = setup_provider().await;
/// assert!(provider.is_some());
/// # }
/// ```
async fn setup_provider() -> Arc<dyn Provider> {
    let ethereum_rpc_url = get_ethereum_rpc_url();
    let url = Url::parse(&ethereum_rpc_url).expect("Invalid URL");

    let builder = ProviderBuilder::new();
    let provider = builder.on_http(url);
    Arc::new(provider)
}

/// Retrieves the Ethereum RPC URL from the environment variables.
///
/// # Examples
///
/// ```rust
/// # fn test_get_ethereum_rpc_url() {
/// let url = get_ethereum_rpc_url();
/// assert!(!url.is_empty());
/// # }
/// ```
fn get_ethereum_rpc_url() -> String {
    env::var("ETHEREUM_RPC_URL").unwrap_or_else(|_| {
        error!("ETHEREUM_RPC_URL not set, using default");
        "http://localhost:8545".to_string()
    })
}

/// Configures CORS for the server.
///
/// # Examples
///
/// ```rust
/// # fn test_setup_cors() {
/// let cors = setup_cors();
/// assert!(cors.is_some());
/// # }
/// ```
fn setup_cors() -> warp::cors::Builder {
    warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE"])
        .allow_headers(vec!["Content-Type", "Authorization"])
}

/// Health check route.
///
/// # Examples
///
/// ```rust
/// # fn test_health_check() {
/// let response = health_check().await;
/// assert_eq!(response, "OK");
/// # }
/// ```
async fn health_check() -> Result<impl Reply, Rejection> {
    Ok("OK")
}

/// Sets up the routes for the server.
///
/// # Examples
///
/// ```rust
/// # fn test_setup_routes() {
/// let provider = Arc::new(...); // Mock or create a provider
/// let routes = setup_routes(provider);
/// assert!(routes.is_some());
/// # }
/// ```
fn setup_routes(
    provider: Arc<dyn Provider>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let health_route = warp::path!("health")
        .and(warp::get())
        .and_then(health_check);

    let balance_route = warp::path!("balance" / String)
        .and(warp::get())
        .and(with_provider(provider.clone()))
        .and_then(get_balance);

    balance_route
        .with(warp::log::custom(log_request))
        .or(health_route)
}

/// Logs the details of the request.
///
/// # Examples
///
/// ```rust
/// # fn test_log_request() {
/// let info = ...; // Mock or create a request info
/// log_request(info);
/// # }
/// ```
fn log_request(info: warp::log::Info) {
    let method = info.method();
    let path = info.path();
    let status = info.status();
    let elapsed = info.elapsed();
    let ip = info
        .remote_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| "Unknown".into());

    if status.is_success() {
        info!(
            "Request: {} {} from {} took {:?}",
            method, path, ip, elapsed
        );
    } else {
        error!(
            "Request: {} {} from {} failed with status {:?} and took {:?}",
            method, path, ip, status, elapsed
        );
    }
}

/// Provides the provider to the warp filters.
///
/// # Examples
///
/// ```rust
/// # fn test_with_provider() {
/// let provider = Arc::new(...); // Mock or create a provider
/// let filter = with_provider(provider);
/// assert!(filter.is_some());
/// # }
/// ```
fn with_provider(
    provider: Arc<dyn Provider>,
) -> impl Filter<Extract = (Arc<dyn Provider>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || provider.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;
    use warp::http::StatusCode;
    use warp::test::request;

    use alloy::providers::{Provider, ProviderCall, RootProvider, RpcWithBlock};

    use std::str::FromStr;

    struct DummyProvider;

    impl Provider for DummyProvider {
        fn get_balance<'a>(
            &'a self,
            _address: alloy_primitives::Address,
        ) -> RpcWithBlock<alloy_primitives::Address, alloy_primitives::Uint<256, 4>> {
            RpcWithBlock::new_provider(|_block_id| {
                // Parse "1000" into the expected type.
                let dummy_balance = alloy_primitives::Uint::<256, 4>::from_str("1000")
                    .expect("failed to parse dummy balance");
                ProviderCall::ready(Ok(dummy_balance))
            })
        }

        fn root(&self) -> &RootProvider {
            unimplemented!("DummyProvider does not support `root`")
        }
    }

    #[tokio::test]
    async fn test_get_balance() {
        let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
        let api = warp::path!("balance" / String)
            .and(warp::get())
            .and(super::with_provider(provider.clone()))
            .and_then(get_balance);

        // Use a valid dummy Ethereum address.
        let address = "0x0000000000000000000000000000000000000000";

        let resp = request()
            .method("GET")
            .path(&format!("/balance/{}", address))
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
        // Further assertions can be made by parsing the JSON response.
    }
}
