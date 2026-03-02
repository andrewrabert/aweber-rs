use std::fmt;

#[derive(Clone, Debug)]
pub struct Client {
    pub(crate) baseurl: String,
    pub(crate) client: reqwest::Client,
}

impl Client {
    pub fn new(baseurl: &str) -> Self {
        let client = reqwest::ClientBuilder::new()
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap();
        Self::new_with_client(baseurl, client)
    }

    pub fn new_with_client(baseurl: &str, client: reqwest::Client) -> Self {
        Self {
            baseurl: baseurl.to_string(),
            client,
        }
    }
}

/// Error type for API requests.
#[derive(Debug)]
pub enum ApiError {
    /// HTTP error response with status code and body.
    Http { status: u16, body: String },
    /// Transport or connection error.
    Request(reqwest::Error),
    /// Failed to deserialize the response body.
    Deserialize {
        source: serde_json::Error,
        body: String,
    },
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Http { status, body } => write!(f, "HTTP {status}: {body}"),
            ApiError::Request(e) => write!(f, "request error: {e}"),
            ApiError::Deserialize { source, body } => {
                write!(f, "deserialize error: {source}\nbody: {body}")
            }
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ApiError::Request(e) => Some(e),
            ApiError::Deserialize { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        ApiError::Request(e)
    }
}

enum Body {
    Json(serde_json::Value),
    Form(serde_json::Value),
}

/// A builder for API requests that handles path, query params, and body.
pub struct ApiRequest<'a> {
    client: &'a Client,
    method: reqwest::Method,
    path: String,
    query: Vec<(&'static str, String)>,
    body: Option<Body>,
    extra_headers: Vec<(reqwest::header::HeaderName, String)>,
}

impl<'a> ApiRequest<'a> {
    pub fn new(client: &'a Client, method: reqwest::Method, path: String) -> Self {
        Self {
            client,
            method,
            path,
            query: Vec::new(),
            body: None,
            extra_headers: Vec::new(),
        }
    }

    /// Add an optional query parameter (skipped if None).
    pub fn query_opt<V: std::fmt::Display>(
        mut self,
        key: &'static str,
        value: Option<V>,
    ) -> Self {
        if let Some(v) = value {
            self.query.push((key, v.to_string()));
        }
        self
    }

    /// Set the JSON request body.
    pub fn json_body(mut self, body: impl serde::Serialize) -> Self {
        self.body = Some(Body::Json(
            serde_json::to_value(body).expect("failed to serialize body"),
        ));
        self
    }

    /// Set the form-urlencoded request body.
    pub fn form_body(mut self, body: impl serde::Serialize) -> Self {
        self.body = Some(Body::Form(
            serde_json::to_value(body).expect("failed to serialize body"),
        ));
        self
    }

    /// Add an extra header.
    pub fn header(mut self, name: reqwest::header::HeaderName, value: String) -> Self {
        self.extra_headers.push((name, value));
        self
    }

    fn build_request(self) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.client.baseurl, self.path);
        let mut req = self.client.client.request(self.method, &url);
        req = req.header(reqwest::header::ACCEPT, "application/json");
        for (k, v) in &self.query {
            req = req.query(&[(k, v)]);
        }
        for (name, value) in &self.extra_headers {
            req = req.header(name, value);
        }
        match self.body {
            Some(Body::Json(v)) => req = req.json(&v),
            Some(Body::Form(v)) => req = req.form(&v),
            None => {}
        }
        req
    }

    /// Send the request and deserialize the response.
    pub async fn send<T: serde::de::DeserializeOwned>(self) -> Result<T, ApiError> {
        let response = self.build_request().send().await?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            let body = response.text().await?;
            serde_json::from_str(&body).map_err(|e| ApiError::Deserialize {
                source: e,
                body,
            })
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(ApiError::Http { status, body })
        }
    }

    /// Send the request, ignoring the response body (for DELETE, etc.).
    pub async fn send_no_body(self) -> Result<(), ApiError> {
        let response = self.build_request().send().await?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            Ok(())
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(ApiError::Http { status, body })
        }
    }
}

/// Percent-encode a path segment.
pub fn encode_path(s: &str) -> String {
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('/', "%2F")
        .replace('?', "%3F")
        .replace('#', "%23")
}
