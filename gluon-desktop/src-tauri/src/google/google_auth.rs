use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use tauri::State;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

const REDIRECT_PORT: u16 = 8744;
const REDIRECT_URI: &str = "http://localhost:8744/oauth/callback";

// Google OAuth endpoints
const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

// Required scopes for Google Drive
const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GoogleCredentials {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GoogleTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: i64, // Unix timestamp
}

#[derive(Clone)]
pub struct GoogleAuthState {
    credentials: Arc<Mutex<Option<GoogleCredentials>>>,
    pending_auth: Arc<Mutex<Option<PendingAuth>>>,
    // FALLBACK: Przechowuj tokeny w RAM jeśli keyring zawiedzie
    pub tokens: Arc<Mutex<Option<GoogleTokens>>>,
}

struct PendingAuth {
    csrf_token: String,
    pkce_verifier: String,
}

impl GoogleAuthState {
    pub fn new() -> Self {
        Self {
            credentials: Arc::new(Mutex::new(None)),
            pending_auth: Arc::new(Mutex::new(None)),
            tokens: Arc::new(Mutex::new(None)),
        }
    }
}

// File persistence helpers
#[derive(Serialize, Deserialize, Default)]
struct AuthPersistence {
    credentials: Option<GoogleCredentials>,
    tokens: Option<GoogleTokens>,
}

fn get_persistence_path() -> std::path::PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or(std::path::PathBuf::from("."));
    path.push("gluon-v2");
    std::fs::create_dir_all(&path).unwrap_or_default();
    path.push("google_auth_store.json");
    path
}

fn save_to_disk(creds: &Option<GoogleCredentials>, tokens: &Option<GoogleTokens>) -> Result<(), String> {
    let data = AuthPersistence {
        credentials: creds.clone(),
        tokens: tokens.clone(),
    };
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    let path = get_persistence_path();
    std::fs::write(&path, json).map_err(|e| format!("Failed to write auth file: {}", e))?;
    println!("[GoogleAuth] 💾 Saved auth data to {:?}", path);
    Ok(())
}

fn load_from_disk() -> Result<AuthPersistence, String> {
    let path = get_persistence_path();
    if !path.exists() {
        return Ok(AuthPersistence::default());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

/// Save credentials to state AND disk
#[tauri::command]
pub async fn set_google_credentials(
    client_id: String,
    client_secret: String,
    state: State<'_, GoogleAuthState>,
) -> Result<(), String> {
    let credentials = GoogleCredentials {
        client_id,
        client_secret,
    };

    // 1. Save to Memory
    let mut creds = state.credentials.lock().map_err(|e| e.to_string())?;
    *creds = Some(credentials.clone());

    // 2. Save to Disk (preserving existing tokens if any)
    let tokens = state.tokens.lock().map_err(|e| e.to_string())?.clone();
    save_to_disk(&Some(credentials), &tokens)?;

    Ok(())
}

/// Check if credentials are set (load from disk if needed)
#[tauri::command]
pub async fn has_google_credentials(state: State<'_, GoogleAuthState>) -> Result<bool, String> {
    let mut creds = state.credentials.lock().map_err(|e| e.to_string())?;
    
    // If not in memory, try loading from disk
    if creds.is_none() {
        if let Ok(data) = load_from_disk() {
            if let Some(loaded_creds) = data.credentials {
                *creds = Some(loaded_creds);
                // Also restore tokens if we loaded from disk
                if let Ok(mut tokens_guard) = state.tokens.lock() {
                    *tokens_guard = data.tokens;
                }
            }
        }
    }
    
    Ok(creds.is_some())
}

/// Start OAuth login flow
#[tauri::command]
pub async fn start_google_login(state: State<'_, GoogleAuthState>) -> Result<String, String> {
    // Get credentials from state
    let credentials = {
        let creds = state.credentials.lock().map_err(|e| e.to_string())?;
        creds.clone().ok_or("Google credentials not set. Please configure Client ID and Secret first.")?
    };

    // Create OAuth client
    let client = BasicClient::new(
        ClientId::new(credentials.client_id),
        Some(ClientSecret::new(credentials.client_secret)),
        AuthUrl::new(GOOGLE_AUTH_URL.to_string()).map_err(|e| e.to_string())?,
        Some(TokenUrl::new(GOOGLE_TOKEN_URL.to_string()).map_err(|e| e.to_string())?),
    )
    .set_redirect_uri(RedirectUrl::new(REDIRECT_URI.to_string()).map_err(|e| e.to_string())?);

    // Generate PKCE challenge
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate authorization URL
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(DRIVE_SCOPE.to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // Store pending auth state
    let mut pending = state.pending_auth.lock().map_err(|e| e.to_string())?;
    *pending = Some(PendingAuth {
        csrf_token: csrf_token.secret().to_string(),
        pkce_verifier: pkce_verifier.secret().to_string(),
    });

    // Start local callback server in background
    let state_clone = state.inner().clone();
    tokio::spawn(async move {
        if let Err(e) = run_callback_server(state_clone).await {
            eprintln!("OAuth callback server error: {}", e);
        }
    });

    // Open browser
    let url_string = auth_url.to_string();
    if let Err(e) = open::that(&url_string) {
        eprintln!("Failed to open browser: {}", e);
    }

    Ok(url_string)
}

/// Run local HTTP server to handle OAuth callback
async fn run_callback_server(state: GoogleAuthState) -> Result<(), String> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT))
        .map_err(|e| format!("Failed to bind callback server: {}", e))?;

    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set non-blocking: {}", e))?;

    let listener = tokio::net::TcpListener::from_std(listener)
        .map_err(|e| format!("Failed to create tokio listener: {}", e))?;

    // Accept single connection (timeout after 5 minutes)
    let timeout = tokio::time::Duration::from_secs(300);
    let result = tokio::time::timeout(timeout, listener.accept()).await;

    match result {
        Ok(Ok((stream, _))) => {
            handle_callback(stream, state).await?;
        }
        Ok(Err(e)) => return Err(format!("Failed to accept connection: {}", e)),
        Err(_) => return Err("OAuth timeout - no response received".to_string()),
    }

    Ok(())
}

/// Handle OAuth callback request
async fn handle_callback(mut stream: TcpStream, state: GoogleAuthState) -> Result<(), String> {
    let buf_reader = BufReader::new(&mut stream);
    let mut lines = buf_reader.lines();

    let request_line = lines
        .next_line()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Empty request")?;

    // Parse query parameters from request line
    let params = parse_callback_params(&request_line)?;

    // Verify CSRF token
    let pending = {
        let mut p = state.pending_auth.lock().map_err(|e| e.to_string())?;
        p.take().ok_or("No pending authentication")?
    };

    let state_param = params.get("state").ok_or("Missing state parameter")?;
    if state_param != &pending.csrf_token {
        return Err("CSRF token mismatch".to_string());
    }

    let code = params.get("code").ok_or("Missing authorization code")?;

    // Exchange code for tokens
    let credentials = {
        let creds = state.credentials.lock().map_err(|e| e.to_string())?;
        creds.clone().ok_or("Credentials not found")?
    };

    let tokens = exchange_code_for_tokens(&credentials, code, &pending.pkce_verifier).await?;

    println!("[GoogleAuth] 🔑 Tokens received from Google. Saving...");

    // 1. Store in RAM (Fallback - 100% pewności działania w sesji)
    if let Ok(mut ram_store) = state.tokens.lock() {
        *ram_store = Some(tokens.clone());
        println!("[GoogleAuth] ✅ Tokens saved to IN-MEMORY cache.");
    }

    // 2. Store in keyring (Persistence)
    match store_tokens(&tokens) {
        Ok(_) => println!("[GoogleAuth] ✅ Tokens saved to System Keyring."),
        Err(e) => println!("[GoogleAuth] ⚠️ Keyring save failed (using RAM fallback): {}", e),
    }

    // Send success response to browser
    let response = "HTTP/1.1 200 OK\r\n\r\n\
        <html><body>\
        <h1>Authentication Successful!</h1>\
        <p>You can close this window and return to Gluon.</p>\
        <script>window.close();</script>\
        </body></html>";

    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Parse callback URL parameters
fn parse_callback_params(request_line: &str) -> Result<HashMap<String, String>, String> {
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err("Invalid request line".to_string());
    }

    let path = parts[1];
    let url = url::Url::parse(&format!("http://localhost{}", path)).map_err(|e| e.to_string())?;

    let mut params = HashMap::new();
    for (key, value) in url.query_pairs() {
        params.insert(key.to_string(), value.to_string());
    }

    Ok(params)
}

/// Exchange authorization code for access and refresh tokens
async fn exchange_code_for_tokens(
    credentials: &GoogleCredentials,
    code: &str,
    pkce_verifier: &str,
) -> Result<GoogleTokens, String> {
    let client = BasicClient::new(
        ClientId::new(credentials.client_id.clone()),
        Some(ClientSecret::new(credentials.client_secret.clone())),
        AuthUrl::new(GOOGLE_AUTH_URL.to_string()).map_err(|e| e.to_string())?,
        Some(TokenUrl::new(GOOGLE_TOKEN_URL.to_string()).map_err(|e| e.to_string())?),
    )
    .set_redirect_uri(RedirectUrl::new(REDIRECT_URI.to_string()).map_err(|e| e.to_string())?);

    let pkce_verifier = oauth2::PkceCodeVerifier::new(pkce_verifier.to_string());

    let token_result = client
        .exchange_code(AuthorizationCode::new(code.to_string()))
        .set_pkce_verifier(pkce_verifier)
        .request_async(async_http_client)
        .await
        .map_err(|e| format!("Token exchange failed: {}", e))?;

    let access_token = token_result.access_token().secret().to_string();
    let refresh_token = token_result.refresh_token().map(|t| t.secret().to_string());

    let expires_in = token_result.expires_in().unwrap_or(std::time::Duration::from_secs(3600));
    let expires_at = chrono::Utc::now().timestamp() + expires_in.as_secs() as i64;

    Ok(GoogleTokens {
        access_token,
        refresh_token,
        expires_at,
    })
}

/// Store tokens (now uses File Persistence + RAM, Keyring removed as it was unstable)
fn store_tokens(tokens: &GoogleTokens) -> Result<(), String> {
    // We can't access `state` here easily to get credentials, so we load from disk first
    let mut current_data = load_from_disk().unwrap_or_default();
    
    // Update tokens
    current_data.tokens = Some(tokens.clone());
    
    // Save back to disk
    save_to_disk(&current_data.credentials, &current_data.tokens)
}

/// Retrieve tokens from File Persistence
fn get_stored_tokens() -> Result<GoogleTokens, String> {
    let data = load_from_disk()?;
    data.tokens.ok_or("No tokens found on disk".to_string())
}

/// Get valid access token (refresh if needed)
#[tauri::command]
pub async fn get_google_access_token(
    state: State<'_, GoogleAuthState>,
) -> Result<String, String> {
    // 1. Try Keyring
    let mut tokens = match get_stored_tokens() {
        Ok(t) => t,
        Err(_) => {
            // 2. Try RAM Fallback
            let ram_store = state.tokens.lock().map_err(|e| e.to_string())?;
            ram_store.clone().ok_or("No stored access token found (RAM or Keyring)")?
        }
    };

    // Check if token is expired (with 5-minute buffer)
    let now = chrono::Utc::now().timestamp();
    let buffer = 300; // 5 minutes

    if now + buffer >= tokens.expires_at {
        // Token expired, refresh it
        let credentials = {
            let creds = state.credentials.lock().map_err(|e| e.to_string())?;
            creds.clone().ok_or("Credentials not set")?
        };

        let refresh_token = tokens.refresh_token.ok_or("No refresh token available")?;
        tokens = refresh_access_token(&credentials, &refresh_token).await?;
        store_tokens(&tokens)?;
    }

    Ok(tokens.access_token)
}

/// Refresh access token using refresh token
async fn refresh_access_token(
    credentials: &GoogleCredentials,
    refresh_token: &str,
) -> Result<GoogleTokens, String> {
    let client = BasicClient::new(
        ClientId::new(credentials.client_id.clone()),
        Some(ClientSecret::new(credentials.client_secret.clone())),
        AuthUrl::new(GOOGLE_AUTH_URL.to_string()).map_err(|e| e.to_string())?,
        Some(TokenUrl::new(GOOGLE_TOKEN_URL.to_string()).map_err(|e| e.to_string())?),
    );

    let refresh_token = oauth2::RefreshToken::new(refresh_token.to_string());

    let token_result = client
        .exchange_refresh_token(&refresh_token)
        .request_async(async_http_client)
        .await
        .map_err(|e| format!("Token refresh failed: {}", e))?;

    let access_token = token_result.access_token().secret().to_string();
    let new_refresh_token = token_result
        .refresh_token()
        .map(|t| t.secret().to_string())
        .or_else(|| Some(refresh_token.secret().to_string())); // Keep old refresh token if not returned

    let expires_in = token_result.expires_in().unwrap_or(std::time::Duration::from_secs(3600));
    let expires_at = chrono::Utc::now().timestamp() + expires_in.as_secs() as i64;

    Ok(GoogleTokens {
        access_token,
        refresh_token: new_refresh_token,
        expires_at,
    })
}

/// Check if user is logged in (has valid tokens)
#[tauri::command]
pub async fn is_google_logged_in(state: State<'_, GoogleAuthState>) -> Result<bool, String> {
    // 1. Check RAM first (Fastest & most reliable in dev)
    if let Ok(ram_store) = state.tokens.lock() {
        if ram_store.is_some() {
            // println!("[GoogleAuth] Status Check: Logged in (RAM)");
            return Ok(true);
        }
    }

    // 2. Check Keyring (Persistence)
    match get_stored_tokens() {
        Ok(_) => {
            // println!("[GoogleAuth] Status Check: Logged in (Keyring)");
            Ok(true)
        },
        Err(_) => Ok(false),
    }
}

/// Logout - clear tokens from disk and memory
#[tauri::command]
pub async fn google_logout(state: State<'_, GoogleAuthState>) -> Result<(), String> {
    // 1. Clear Memory
    if let Ok(mut tokens) = state.tokens.lock() {
        *tokens = None;
    }
    
    // 2. Clear Disk (Keep credentials, remove tokens)
    let mut data = load_from_disk().unwrap_or_default();
    data.tokens = None;
    save_to_disk(&data.credentials, &data.tokens)?;

    println!("[GoogleAuth] 🚪 Logged out successfully.");
    Ok(())
}