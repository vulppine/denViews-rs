use crate::servers::routing::api::{APIHandler, APIRequest};
use crate::Error;
use hyper::header::{HeaderName, HeaderValue};
use lambda_runtime as lambda;
use std::collections::HashMap;
use std::net::SocketAddr;
// use lambda_runtime::{handler_fn, run, Context};

pub async fn run() {
    lambda::run(lambda::handler_fn(handle)).await.unwrap();
}

#[derive(serde::Deserialize)]
struct LambdaAPIGatewayRequest {
    version: String,
    #[serde(alias = "rawPath")]
    raw_path: String,
    #[serde(alias = "rawQueryString")]
    raw_query_string: String,
    cookies: Vec<String>,
    headers: HashMap<String, String>,
    #[serde(alias = "queryStringParameters")]
    query_string_parameters: HashMap<String, String>,
    #[serde(alias = "requestContext")]
    request_context: LambdaAPIGatewayRequestContext,
    body: String,
    #[serde(alias = "pathParameters")]
    path_parameters: HashMap<String, String>,
    #[serde(alias = "isBase64Encoded")]
    is_base64_encoded: bool,
    #[serde(alias = "stageVariables")]
    stage_variables: HashMap<String, String>,
}

impl Default for LambdaAPIGatewayRequest {
    fn default() -> Self {
        LambdaAPIGatewayRequest {
            version: "2.0".into(),
            raw_path: "".into(),
            raw_query_string: "".into(),
            cookies: Vec::new(),
            headers: HashMap::new(),
            query_string_parameters: HashMap::new(),
            request_context: LambdaAPIGatewayRequestContext::default(),
            body: "".into(),
            path_parameters: HashMap::new(),
            is_base64_encoded: false,
            stage_variables: HashMap::new(),
        }
    }
}

impl LambdaAPIGatewayRequest {
    fn into_request(self) -> Result<hyper::Request<hyper::Body>, Error> {
        let mut req = hyper::Request::builder().method(self.request_context.http.method.as_str());

        for header in self.headers {
            req = req.header(
                HeaderName::from_bytes(header.0.as_bytes())?,
                HeaderValue::from_str(&header.1)?,
            );
        }

        for cookie in self.cookies {
            req = req.header(hyper::header::COOKIE, HeaderValue::from_str(&cookie)?);
        }

        let uri = hyper::Uri::builder()
            .path_and_query(match self.raw_query_string.len() {
                0 => self.raw_path,
                _ => [self.raw_path, self.raw_query_string].join("?"),
            })
            .build()?;

        req = req.uri(uri);

        Ok(req.body(hyper::Body::from(self.body.clone()))?)
    }
}

/*
impl From<hyper::Request<hyper::Body>> for LambdaAPIGatewayRequest {
    fn from(req: hyper::Request<hyper::Body>) -> Self {
        let mut result = LambdaAPIGatewayRequest::default();
        result.request_context.http.method = req.method().as_str().into();
        let header_map = req.headers().into_iter();
        for header in header_map {
            result
                .headers
                .insert(header.0.as_str().into(), header.1.to_str().unwrap().into());
        }
        let uri = req.uri();
        let path_query = uri.path_and_query();
        result.raw_path = uri.path().into();
        result.request_context.http.path = uri.path().into();
        result.raw_query_string = match uri.query() {
            None => "".into(),
            Some(q) => q.into(),
        };
        // jank but it's the only way this works
        result.request_context.http.protocol = format!("{:?}", req.version());

        let (tx, rx) = oneshot::channel::<String>();
        async move {
            let res: String = hyper::body::to_bytes(req.into_body()).await.unwrap().into();
            tx.send(res);
        };

        result.body = rx.try_recv

        result
    }
}
*/

#[derive(serde::Deserialize)]
struct LambdaAPIGatewayRequestContext {
    #[serde(alias = "accountId")]
    account_id: String,
    #[serde(alias = "apiId")]
    api_id: String,
    #[serde(alias = "domainName")]
    domain_name: String,
    #[serde(alias = "domainPrefix")]
    domain_prefix: String,
    http: LambdaAPIGatewayRequestContextHTTP,
    #[serde(alias = "requestId")]
    request_id: String,
    #[serde(alias = "routeKey")]
    route_key: String,
    stage: String,
    time: String,
    #[serde(alias = "timeEpoch")]
    time_epoch: u64,
}

impl Default for LambdaAPIGatewayRequestContext {
    fn default() -> Self {
        LambdaAPIGatewayRequestContext {
            account_id: "".into(),
            api_id: "".into(),
            domain_name: "".into(),
            domain_prefix: "".into(),
            http: LambdaAPIGatewayRequestContextHTTP::default(),
            request_id: "".into(),
            route_key: "".into(),
            stage: "".into(),
            time: "".into(),
            time_epoch: 0,
        }
    }
}

#[derive(serde::Deserialize)]
struct LambdaAPIGatewayRequestContextHTTP {
    method: String,
    path: String,
    protocol: String,
    #[serde(alias = "sourceIp")]
    source_ip: String,
    #[serde(alias = "userAgent")]
    user_agent: String,
}

impl Default for LambdaAPIGatewayRequestContextHTTP {
    fn default() -> Self {
        LambdaAPIGatewayRequestContextHTTP {
            method: "".into(),
            path: "".into(),
            protocol: "".into(),
            source_ip: "".into(),
            user_agent: "".into(),
        }
    }
}

#[derive(serde::Serialize)]
struct LambdaAPIGatewayResponse {
    cookies: Vec<String>,
    #[serde(rename = "isBase64Encoded")]
    is_base64_encoded: bool,
    #[serde(rename = "statusCode")]
    status_code: u16,
    headers: HashMap<String, String>,
    body: String,
}

impl LambdaAPIGatewayResponse {
    async fn from_response(res: hyper::Response<hyper::Body>) -> Result<Self, Error> {
        let mut headers: HashMap<String, String> = HashMap::new();
        let mut cookies: Vec<String> = Vec::new();
        for header in res.headers() {
            if header.0 == hyper::header::SET_COOKIE {
                cookies.push(header.1.to_str()?.into());
                continue;
            }
            headers.insert(header.0.as_str().into(), header.1.to_str()?.into());
        }

        let status_code = res.status().as_u16();
        let body = String::from_utf8(hyper::body::to_bytes(res.into_body()).await?.to_vec())?;

        Ok(LambdaAPIGatewayResponse {
            cookies,
            is_base64_encoded: false,
            status_code,
            headers,
            body,
        })
    }
}

async fn handle(
    req: LambdaAPIGatewayRequest,
    _: lambda::Context,
) -> Result<LambdaAPIGatewayResponse, Error> {
    let client = APIHandler::new().await?;
    let ip: SocketAddr = req.request_context.http.source_ip.parse()?;
    let auth = match req.headers().get(hyper::header::AUTHORIZATION) {
        None => false,
        Some(v) => {
            let userpass = String::from_utf8(base64_to_bytes(auth_header[1].into()))?
                .split(':')
                .map(|s| s.into())
                .collect::<Vec<String>>();
            client
                .auth(userpass[0].clone(), userpass[1].clone())
                .await?
        }
    };
    let resp = client
        .execute(APIRequest {
            req: req.into_request()?,
            ip,
            auth,
        })
        .await?;

    Ok(LambdaAPIGatewayResponse::from_response(resp).await?)
}
