use anyhow::{Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CLIENT_ID: &str = "lZ68iB3i3ZKdCI4q9Uwkqx1c4ykiFe3c";
const REDIRECT_URI: &str = "urn:ietf:wg:oauth:2.0:oob";
const AUTH_URL: &str = "https://auth.aweber.com/oauth2/authorize";
const TOKEN_URL: &str = "https://auth.aweber.com/oauth2/token";
const SCOPES: &str = "account.read list.read list.write subscriber.read subscriber.write subscriber.read-extended email.read email.write landing-page.read";

#[derive(Debug, Serialize, Deserialize)]
struct Credentials {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    account_id: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

fn credentials_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().context("could not determine config directory")?;
    Ok(config_dir.join("aweber").join("credentials.json"))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn save_credentials(creds: &Credentials) -> Result<()> {
    let path = credentials_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("could not create config directory")?;
    }
    let json = serde_json::to_string_pretty(creds)?;
    std::fs::write(&path, &json).context("could not write credentials file")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .context("could not set credentials file permissions")?;
    }

    Ok(())
}

fn load_credentials() -> Result<Credentials> {
    let path = credentials_path()?;
    let json = std::fs::read_to_string(&path).context("could not read credentials file")?;
    let creds: Credentials = serde_json::from_str(&json)?;
    Ok(creds)
}

fn generate_code_verifier() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut bytes = [0u8; 32];
    for chunk in bytes.chunks_mut(8) {
        let hash = RandomState::new().build_hasher().finish().to_le_bytes();
        chunk.copy_from_slice(&hash[..chunk.len()]);
    }
    URL_SAFE_NO_PAD.encode(bytes)
}

fn generate_code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

async fn exchange_code(code: &str, code_verifier: &str) -> Result<Credentials> {
    let client = reqwest::Client::new();
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", CLIENT_ID),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .context("failed to contact token endpoint")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("token exchange failed ({status}): {body}");
    }

    let token_resp: TokenResponse = resp
        .json()
        .await
        .context("failed to parse token response")?;
    Ok(Credentials {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token,
        expires_at: now_secs() + token_resp.expires_in,
        account_id: String::new(), // populated after fetching accounts
    })
}

async fn fetch_account_id(access_token: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Account {
        id: i64,
    }
    #[derive(Deserialize)]
    struct Accounts {
        entries: Vec<Account>,
    }

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.aweber.com/1.0/accounts")
        .bearer_auth(access_token)
        .send()
        .await
        .context("failed to fetch accounts")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("failed to fetch accounts ({status}): {body}");
    }

    let accounts: Accounts = resp
        .json()
        .await
        .context("failed to parse accounts response")?;
    let account = accounts
        .entries
        .first()
        .context("no accounts found for this token")?;
    Ok(account.id.to_string())
}

async fn refresh(creds: &Credentials) -> Result<Credentials> {
    let client = reqwest::Client::new();
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &creds.refresh_token),
            ("client_id", CLIENT_ID),
        ])
        .send()
        .await
        .context("failed to contact token endpoint")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("token refresh failed ({status}): {body}");
    }

    let token_resp: TokenResponse = resp
        .json()
        .await
        .context("failed to parse token response")?;
    Ok(Credentials {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token,
        expires_at: now_secs() + token_resp.expires_in,
        account_id: creds.account_id.clone(),
    })
}

pub async fn login() -> Result<()> {
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    let scope_encoded = SCOPES.replace(' ', "%20");
    let url = format!(
        "{AUTH_URL}?response_type=code&client_id={CLIENT_ID}&redirect_uri={REDIRECT_URI}&scope={scope_encoded}&code_challenge={code_challenge}&code_challenge_method=S256"
    );

    println!("Open this URL in your browser to authorize:\n");
    println!("  {url}\n");

    if open::that(&url).is_err() {
        println!("(Could not open browser automatically. Copy the URL above.)");
    }

    println!("After authorizing, paste the code below.\n");
    use std::io::Write;
    print!("Authorization code: ");
    std::io::stdout()
        .flush()
        .context("failed to flush stdout")?;

    let mut code = String::new();
    std::io::stdin()
        .read_line(&mut code)
        .context("failed to read authorization code")?;
    let code = code.trim();

    if code.is_empty() {
        anyhow::bail!("no authorization code provided");
    }

    let mut creds = exchange_code(code, &code_verifier).await?;
    creds.account_id = fetch_account_id(&creds.access_token).await?;
    save_credentials(&creds)?;

    println!(
        "\nLogged in successfully (account {}). Token expires in 2 hours and will auto-refresh.",
        creds.account_id
    );
    Ok(())
}

pub fn logout() -> Result<()> {
    let path = credentials_path()?;
    if path.exists() {
        std::fs::remove_file(&path).context("could not delete credentials file")?;
        println!("Logged out.");
    } else {
        println!("Not logged in.");
    }
    Ok(())
}

pub fn status() -> Result<()> {
    match load_credentials() {
        Ok(creds) => {
            let now = now_secs();
            if creds.expires_at > now {
                let remaining = creds.expires_at - now;
                let mins = remaining / 60;
                println!(
                    "Logged in (account {}). Token expires in {mins} minutes.",
                    creds.account_id
                );
            } else {
                println!(
                    "Logged in (account {}). Token expired (will auto-refresh on next API call).",
                    creds.account_id
                );
            }
        }
        Err(_) => {
            println!("Not logged in. Run `aweber auth login` to authenticate.");
        }
    }
    Ok(())
}

/// Load a valid access token and account ID from stored credentials, refreshing if expired.
pub async fn load_session() -> Result<(String, String)> {
    let creds = load_credentials().context("not logged in — run `aweber auth login` first")?;

    const EXPIRY_BUFFER_SECS: u64 = 60;
    if creds.expires_at > now_secs() + EXPIRY_BUFFER_SECS {
        return Ok((creds.access_token, creds.account_id));
    }

    let new_creds = refresh(&creds).await?;
    save_credentials(&new_creds)?;
    Ok((new_creds.access_token, new_creds.account_id))
}
