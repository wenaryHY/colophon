use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TurnstileResponse {
    pub success: bool,
    #[serde(rename = "error-codes")]
    #[allow(dead_code)]
    pub error_codes: Option<Vec<String>>,
}

pub async fn verify_turnstile(token: &str, secret: &str) -> bool {
    let client = reqwest::Client::new();
    let res = client
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&[("secret", secret), ("response", token)])
        .send()
        .await;

    match res {
        Ok(r) => r
            .json::<TurnstileResponse>()
            .await
            .map(|r| r.success)
            .unwrap_or(false),
        Err(_) => false,
    }
}
