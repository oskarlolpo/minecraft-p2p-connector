use serde::{Deserialize, Serialize};
use worker::*;

const CLOUDFLARE_TURN_API_BASE: &str = "https://rtc.live.cloudflare.com/v1/turn/keys";
const MIN_TTL: u64 = 300;
const MAX_TTL: u64 = 86_400;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateIceServerRequest {
    ttl: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IceServer {
    urls: Vec<String>,
    username: Option<String>,
    credential: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IceServerResponse {
    ice_servers: Vec<IceServer>,
    ttl: Option<u64>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudflareApiResponse {
    ice_servers: Vec<IceServer>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    error: String,
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .get_async("/ice-servers", handle_ice_servers)
        .post_async("/ice-servers", handle_ice_servers)
        .run(req, env)
        .await
}

async fn handle_ice_servers(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let key_id = ctx.secret("TURN_KEY_ID")?.to_string();
    let api_token = ctx.secret("TURN_KEY_API_TOKEN")?.to_string();
    let default_ttl = ctx
        .var("TURN_CREDENTIAL_TTL")
        .ok()
        .and_then(|value| value.to_string().parse::<u64>().ok())
        .unwrap_or(3600);

    let requested_ttl = match req.method() {
        Method::Post => req
            .json::<GenerateIceServerRequest>()
            .await
            .ok()
            .and_then(|payload| payload.ttl),
        _ => req
            .url()?
            .query_pairs()
            .find(|(key, _)| key == "ttl")
            .and_then(|(_, value)| value.parse::<u64>().ok()),
    };

    let ttl = requested_ttl.unwrap_or(default_ttl).clamp(MIN_TTL, MAX_TTL);
    let upstream_url = format!(
        "{CLOUDFLARE_TURN_API_BASE}/{key_id}/credentials/generate-ice-servers"
    );

    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {api_token}"))?;
    headers.set("Content-Type", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(
        serde_json::json!({ "ttl": ttl }).to_string().into(),
    ));

    let upstream_request = Request::new_with_init(&upstream_url, &init)?;
    let mut upstream_response = Fetch::Request(upstream_request).send().await?;

    if !(200..=299).contains(&upstream_response.status_code()) {
        let body = upstream_response
            .text()
            .await
            .unwrap_or_else(|_| "Cloudflare TURN API request failed".into());
        return json_error(
            format!(
                "Cloudflare TURN API returned {}: {}",
                upstream_response.status_code(),
                body
            ),
            upstream_response.status_code(),
        );
    }

    let cloudflare_payload = upstream_response.json::<CloudflareApiResponse>().await?;
    let mut response = Response::from_json(&IceServerResponse {
        ice_servers: cloudflare_payload.ice_servers,
        ttl: Some(ttl),
        note: Some("Cloudflare TURN credentials issued by workers-rs backend.".into()),
    })?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    Ok(response)
}

fn json_error(message: String, status: u16) -> Result<Response> {
    let mut response = Response::from_json(&ErrorResponse { error: message })?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    Ok(response.with_status(status))
}
