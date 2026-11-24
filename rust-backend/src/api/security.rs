//! Native `/api/v1` handlers: SSH private keys (Laravel `SecurityController`) and DNS/secret-manager
//! integration tokens (`IntegrationTokensController`, read + delete).
//!
//! Integration token create/update and zone sync validate the token against the provider's API, so those
//! routes stay in Laravel (the router forwards the methods it does not register).

#[allow(unused_imports)]
use super::prelude::*;

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};

use axum::{body::Body, extract::Request};
use base64::Engine as _;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

// =====================================================================================================
// Shared: Laravel model JSON with `makeHidden` / `makeVisible` and the `encrypted` cast
// =====================================================================================================

/// One Eloquent model serialized like `serializeApiResponse($model)`: `hidden` keys removed (unless in
/// `visible`), visible `encrypted` columns decrypted, dates in Laravel format.
/// `None` when a visible encrypted value cannot be decrypted (Laravel answers 500): delegate.
pub(crate) fn credential_json(
    row: Value,
    spec: &ModelSpec,
    visible: &[&str],
    encrypted: &[&str],
    app_key: Option<&[u8; 32]>,
) -> Option<Map<String, Value>> {
    let Value::Object(mut map) = row else {
        return Some(Map::new());
    };
    for field in encrypted {
        if !visible.contains(field) {
            continue;
        }
        if let Some(Value::String(cipher)) = map.get(*field) {
            let plain = crate::crypto::decrypt(cipher, app_key?)?;
            map.insert((*field).to_string(), Value::String(plain));
        }
    }
    let hidden: Vec<&str> = spec
        .hidden
        .iter()
        .copied()
        .filter(|h| !visible.contains(h))
        .collect();
    map.retain(|k, _| !hidden.contains(&k.as_str()));
    let spec = ModelSpec {
        hidden: &[],
        ..*spec
    };
    match to_laravel(Value::Object(map), &spec) {
        Value::Object(m) => Some(m),
        _ => Some(Map::new()),
    }
}

/// Split a request so its body can be parsed and the request still forwarded to Laravel unchanged.
pub(crate) async fn split_request(req: Request) -> (axum::http::request::Parts, axum::body::Bytes) {
    let (parts, body) = req.into_parts();
    let bytes = axum::body::to_bytes(body, 16 * 1024 * 1024)
        .await
        .unwrap_or_default();
    (parts, bytes)
}

/// Forward a request whose body was already read to Laravel.
pub(crate) async fn forward(
    state: crate::AppState,
    parts: axum::http::request::Parts,
    bytes: axum::body::Bytes,
) -> Response {
    crate::proxy(State(state), Request::from_parts(parts, Body::from(bytes))).await
}

fn unprocessable(body: Value) -> Response {
    (StatusCode::UNPROCESSABLE_ENTITY, Json(body)).into_response()
}

// =====================================================================================================
// Private keys: phpseclib-equivalent parsing through `ssh-keygen`
// =====================================================================================================

const INVALID_KEY: &str =
    "The private key is invalid. Paste an unencrypted private key, not a public key or a certificate.";
const PUTTY_KEY: &str =
    "PuTTY and other key formats are not supported. Paste the key in OpenSSH or PEM format. It starts with \"-----BEGIN\".";
const DSA_KEY: &str = "DSA keys are not supported. Use an Ed25519 or RSA key.";
const SHORT_RSA_KEY: &str = "RSA keys must have at least 2048 bits.";
const SUPPORTED_TYPES: &[&str] = &[
    "ssh-ed25519",
    "ssh-rsa",
    "ecdsa-sha2-nistp256",
    "ecdsa-sha2-nistp384",
    "ecdsa-sha2-nistp521",
];

/// What phpseclib derives from a private key.
#[derive(Clone, Debug, PartialEq, Eq)]
struct KeyInfo {
    key_type: String,
    /// `toString('OpenSSH', ['comment' => ''])`: "<type> <base64> " (empty comment, trailing space).
    public_key: String,
    /// `getFingerprint('sha256')`: base64(sha256(blob)) without its last character.
    fingerprint: String,
    rsa_bits: Option<usize>,
}

/// Parse a private key with `ssh-keygen -y` (OpenSSH, PKCS#1, PKCS#8, SEC1). Cached per key content.
// ponytail: one ssh-keygen process per distinct key; a native parser (ssh-key crate) if this gets hot.
async fn key_info(private_key: &str) -> Option<KeyInfo> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<KeyInfo>>>> = OnceLock::new();
    let digest = hex::encode(Sha256::digest(private_key.as_bytes()));
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&digest) {
        return hit.clone();
    }
    let info = run_ssh_keygen(private_key).await;
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    if guard.len() > 1000 {
        guard.clear();
    }
    guard.insert(digest, info.clone());
    info
}

async fn run_ssh_keygen(private_key: &str) -> Option<KeyInfo> {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!(
        "tinyhelm-keyprobe-{}-{}",
        std::process::id(),
        new_public_id()
    ));
    std::fs::create_dir(&dir).ok()?;
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    let path = dir.join("key");
    let mut content = private_key.to_string();
    if !content.ends_with('\n') {
        content.push('\n');
    }
    let written = std::fs::write(&path, content)
        .and_then(|_| std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)));
    let output = match written {
        Ok(()) => tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::process::Command::new("ssh-keygen")
                .args(["-y", "-P", "", "-f"])
                .arg(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .ok()
        .and_then(Result::ok),
        Err(_) => None,
    };
    let _ = std::fs::remove_dir_all(&dir);
    let output = output.filter(|o| o.status.success())?;
    parse_public_line(String::from_utf8_lossy(&output.stdout).lines().next()?)
}

fn parse_public_line(line: &str) -> Option<KeyInfo> {
    let mut fields = line.split_whitespace();
    let key_type = fields.next()?.to_string();
    let b64 = fields.next()?;
    let blob = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let digest = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(&blob));
    let fingerprint = digest[..digest.len() - 1].to_string();
    let rsa_bits = (key_type == "ssh-rsa")
        .then(|| rsa_modulus_bits(&blob))
        .flatten();
    Some(KeyInfo {
        public_key: format!("{key_type} {b64} "),
        key_type,
        fingerprint,
        rsa_bits,
    })
}

/// Bit length of the modulus in an `ssh-rsa` public key blob (string type, mpint e, mpint n).
fn rsa_modulus_bits(blob: &[u8]) -> Option<usize> {
    let mut rest = blob;
    let mut take = || -> Option<&[u8]> {
        let len = u32::from_be_bytes(rest.get(..4)?.try_into().ok()?) as usize;
        let field = rest.get(4..4 + len)?;
        rest = &rest[4 + len..];
        Some(field)
    };
    take()?;
    take()?;
    let n = take()?;
    let first = n.iter().position(|b| *b != 0)?;
    Some((n.len() - first) * 8 - n[first].leading_zeros() as usize)
}

/// `PrivateKey::validationError($key, strict: true)`.
async fn key_validation_error(private_key: &str) -> Option<&'static str> {
    let Some(info) = key_info(private_key).await else {
        // phpseclib loads unencrypted PuTTY and DSA keys that ssh-keygen may not; keep Laravel's messages.
        if private_key.contains("PuTTY-User-Key-File") && private_key.contains("Encryption: none") {
            return Some(PUTTY_KEY);
        }
        if private_key.contains("-----BEGIN DSA PRIVATE KEY-----") {
            return Some(DSA_KEY);
        }
        return Some(INVALID_KEY);
    };
    if !private_key.trim().starts_with("-----BEGIN ") {
        return Some(PUTTY_KEY);
    }
    if info.key_type == "ssh-dss" {
        return Some(DSA_KEY);
    }
    if !SUPPORTED_TYPES.contains(&info.key_type.as_str()) {
        return Some(INVALID_KEY);
    }
    if info.rsa_bits.is_some_and(|b| b < 2048) {
        return Some(SHORT_RSA_KEY);
    }
    None
}

/// `formatPrivateKey()`: trimmed, one trailing newline.
fn format_private_key(key: &str) -> String {
    format!("{}\n", key.trim())
}

/// `generate_random_name()`: alliterating "adjective-noun-<cuid2>" in kebab case.
fn generate_random_name() -> String {
    const PAIRS: &[(&str, &str)] = &[
        ("agile", "antelope"),
        ("brave", "badger"),
        ("calm", "cougar"),
        ("daring", "dolphin"),
        ("eager", "eagle"),
        ("fancy", "falcon"),
        ("gentle", "gecko"),
        ("happy", "heron"),
        ("jolly", "jaguar"),
        ("keen", "koala"),
        ("lively", "lynx"),
        ("mighty", "moose"),
        ("noble", "newt"),
        ("plucky", "panda"),
        ("quick", "quail"),
        ("rapid", "raven"),
        ("swift", "sparrow"),
        ("tidy", "tiger"),
        ("vivid", "viper"),
        ("witty", "walrus"),
    ];
    let id = new_public_id();
    let (adjective, noun) = PAIRS[id.bytes().map(usize::from).sum::<usize>() % PAIRS.len()];
    format!("{adjective}-{noun}-{id}")
}

/// `$request->private_key` as the API accepts it: PEM text, or base64 of a key (strict decode).
fn decode_submitted_key(raw: &str) -> String {
    if raw.starts_with("-----BEGIN") {
        return raw.to_string();
    }
    let engine = base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::GeneralPurposeConfig::new()
            .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
    );
    match engine.decode(raw.as_bytes()) {
        // Decoded bytes that are not text are not a key either.
        Ok(bytes) => String::from_utf8(bytes).unwrap_or_default(),
        Err(_) => raw.to_string(),
    }
}

const PRIVATE_KEY_SPEC: ModelSpec = ModelSpec {
    hidden: &["private_key"],
    datetimes: &[],
    dates: &[],
    json_strings: &[],
    no_timestamps: false,
};

/// `customApiValidator(..., ValidationPatterns::combinedMessages())`.
const KEY_MESSAGES: &[(&str, &str)] = &[
    ("required", "This field is required."),
    ("name.regex", "The name may only contain letters (including Unicode), numbers, spaces, and these characters: - _ . / @ & ( ) # , : +"),
    ("name.min", "The name must be at least :min characters."),
    ("name.max", "The name may not be greater than :max characters."),
    ("description.regex", "The description may only contain letters (including Unicode), numbers, spaces, and common punctuation: - _ . , ! ? ( ) ' \" + = * / @ &"),
    ("description.max", "The description may not be greater than :max characters."),
];

/// `nameRules(required: false, nullable: false)`.
const NAME_RULES: &[Rule] = &[
    Rule::Str,
    Rule::Min(3),
    Rule::Max(255),
    Rule::Regex(NAME_PATTERN),
];
const DESCRIPTION_RULES: &[Rule] = &[
    Rule::Nullable,
    Rule::Str,
    Rule::Max(255),
    Rule::Regex(DESCRIPTION_PATTERN),
];
const KEY_COLUMNS: &[&str] = &[
    "id",
    "uuid",
    "name",
    "description",
    "private_key",
    "is_git_related",
    "team_id",
    "created_at",
    "updated_at",
    "fingerprint",
];

/// A key row as the API returns it (with the `public_key` append; `private_key` only when visible).
async fn key_json(state: &crate::AppState, row: Value, show_private: bool) -> Option<Value> {
    let cipher = row
        .get("private_key")
        .and_then(Value::as_str)
        .map(str::to_string);
    let private = match (&cipher, state.app_key.as_ref()) {
        (Some(c), Some(k)) => Some(crate::crypto::decrypt(c, k)?),
        _ => None,
    };
    let visible: &[&str] = if show_private { &["private_key"] } else { &[] };
    let mut map = credential_json(
        row,
        &PRIVATE_KEY_SPEC,
        visible,
        &["private_key"],
        state.app_key.as_ref(),
    )?;
    let public = match private.as_deref() {
        Some(p) => key_info(p).await.map(|i| i.public_key),
        None => None,
    };
    map.insert(
        "public_key".into(),
        Value::String(public.unwrap_or_else(|| "Error loading private key".into())),
    );
    Some(Value::Object(map))
}

/// GET /api/v1/security/keys
pub async fn list_keys(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let rows = match sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(k) FROM private_keys k WHERE team_id = $1 ORDER BY id",
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return query_err("list_keys", e),
    };
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        match key_json(&state, row, ctx.can_read_sensitive()).await {
            Some(v) => out.push(v),
            None => return crate::proxy(State(state.clone()), req).await,
        }
    }
    ok_json(out)
}

/// GET /api/v1/security/keys/:uuid
pub async fn get_key(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match fetch_key_row(pool, team_id, &uuid).await {
        Ok(Some(row)) => match key_json(&state, row, ctx.can_read_sensitive()).await {
            Some(v) => ok_json(v),
            None => crate::proxy(State(state.clone()), req).await,
        },
        Ok(None) => not_found("Private Key not found."),
        Err(e) => query_err("get_key", e),
    }
}

async fn fetch_key_row(
    pool: &PgPool,
    team_id: i64,
    uuid: &str,
) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(k) FROM private_keys k WHERE team_id = $1 AND uuid = $2",
    )
    .bind(team_id)
    .bind(uuid)
    .fetch_optional(pool)
    .await
}

async fn fingerprint_exists(
    pool: &PgPool,
    team_id: i64,
    fingerprint: &str,
    exclude: Option<i64>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM private_keys WHERE team_id = $1 AND fingerprint = $2 \
         AND ($3::bigint IS NULL OR id <> $3))",
    )
    .bind(team_id)
    .bind(fingerprint)
    .bind(exclude)
    .fetch_one(pool)
    .await
}

/// PHP truthiness of a request value (`if (! $request->name)`).
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty() && s != "0",
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(_)) => true,
    }
}

/// POST /api/v1/security/keys
pub async fn create_key(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    req: Request,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (parts, bytes) = split_request(req).await;
    let input = match json_input(&parts.headers, &bytes) {
        Ok(i) => i,
        Err(r) => return r,
    };
    let mut v = Validator::new(&input, KEY_MESSAGES);
    v.field("name", NAME_RULES);
    v.field("description", DESCRIPTION_RULES);
    v.field("private_key", &[Rule::Required, Rule::Str]);
    if v.fails() {
        return v.response();
    }
    let Some(app_key) = state.app_key else {
        return forward(state, parts, bytes).await;
    };

    let name = if truthy(input.get("name")) {
        input_str(&input, "name").unwrap_or_default()
    } else {
        generate_random_name()
    };
    let description = if truthy(input.get("description")) {
        input_str(&input, "description").unwrap_or_default()
    } else {
        "Created by TinyHelm via API".to_string()
    };
    let submitted = decode_submitted_key(&input_str(&input, "private_key").unwrap_or_default());
    if let Some(error) = key_validation_error(&submitted).await {
        return unprocessable(json!({ "message": error }));
    }
    let private_key = format_private_key(&submitted);
    let Some(info) = key_info(&private_key).await else {
        return unprocessable(json!({ "message": INVALID_KEY }));
    };
    match fingerprint_exists(pool, team_id, &info.fingerprint, None).await {
        Ok(true) => return unprocessable(json!({"message": "Private key already exists."})),
        Ok(false) => {}
        Err(e) => return query_err("create_key", e),
    }
    let Some(cipher) = crate::crypto::encrypt_string(&private_key, &app_key) else {
        return forward(state, parts, bytes).await;
    };
    let uuid = new_public_id();
    let inserted = sqlx::query(&format!(
        "INSERT INTO private_keys (uuid, team_id, name, description, private_key, fingerprint, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,{NOW},{NOW})"
    ))
    .bind(&uuid)
    .bind(team_id)
    .bind(&name)
    .bind(&description)
    .bind(&cipher)
    .bind(&info.fingerprint)
    .execute(pool)
    .await;
    if let Err(e) = inserted {
        return query_err("create_key", e);
    }

    // Auditable `created`: the attributes in `create()` + saving-hook order.
    let new = as_map(json!({
        "team_id": team_id, "name": name, "description": description,
        "private_key": "", "fingerprint": info.fingerprint, "uuid": uuid,
    }));
    let changes = audit_changes(None, Some(&new), &fields_of(&new), &["private_key"]);
    record_mutation(
        &state,
        pool,
        &ctx,
        team_id,
        "private_key",
        "created",
        Some(&uuid),
        Some(&name),
        vec![],
        changes,
    )
    .await;
    audit::record(
        pool,
        &ctx,
        "api.private_key.created",
        json!({"team_id": team_id, "private_key_uuid": uuid, "private_key_name": name, "fingerprint": info.fingerprint}),
        "info",
    )
    .await;
    (StatusCode::CREATED, Json(json!({ "uuid": uuid }))).into_response()
}

/// PATCH /api/v1/security/keys/:uuid
pub async fn update_key(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    const ALLOWED: &[&str] = &["name", "description", "private_key"];
    let (pool, team_id) = match ctx.require(&state, "write").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (parts, bytes) = split_request(req).await;
    let input = match json_input(&parts.headers, &bytes) {
        Ok(i) => i,
        Err(r) => return r,
    };
    let mut v = Validator::new(&input, KEY_MESSAGES);
    v.field("name", NAME_RULES);
    v.field("description", DESCRIPTION_RULES);
    v.field("private_key", &[Rule::Str]);
    v.reject_extra(ALLOWED);
    if v.fails() {
        return v.response();
    }
    let Some(app_key) = state.app_key else {
        return forward(state, parts, bytes).await;
    };
    let row = match fetch_key_row(pool, team_id, &uuid).await {
        Ok(Some(r)) => as_map(r),
        Ok(None) => return not_found("Private Key not found."),
        Err(e) => return query_err("update_key", e),
    };
    let id = row.get("id").and_then(Value::as_i64).unwrap_or_default();
    let stored = row
        .get("private_key")
        .and_then(Value::as_str)
        .and_then(|c| crate::crypto::decrypt(c, &app_key));
    let Some(stored) = stored else {
        return forward(state, parts, bytes).await;
    };

    // Eloquent `update()` + the `saving` hook: format, validate a changed key, refresh the fingerprint.
    let mut new = row.clone();
    for field in ["name", "description"] {
        if let Some(value) = input.get(field) {
            new.insert(field.to_string(), value.clone());
        }
    }
    let private_key = match input.get("private_key") {
        Some(Value::String(k)) => format_private_key(k),
        _ => format_private_key(&stored),
    };
    let normalize = |k: &str| format_private_key(k).replace("\r\n", "\n");
    if normalize(&private_key) != normalize(&stored) {
        if let Some(error) = key_validation_error(&private_key).await {
            return unprocessable(json!({"message": error, "errors": {"private_key": [error]}}));
        }
    }
    let fingerprint = key_info(&private_key).await.map(|i| i.fingerprint);
    if let Some(fp) = &fingerprint {
        match fingerprint_exists(pool, team_id, fp, Some(id)).await {
            Ok(true) => {
                let msg = "This private key already exists.";
                return unprocessable(json!({"message": msg, "errors": {"private_key": [msg]}}));
            }
            Ok(false) => {}
            Err(e) => return query_err("update_key", e),
        }
    }
    new.insert(
        "fingerprint".into(),
        fingerprint.clone().map_or(Value::Null, Value::String),
    );
    // The encrypted cast compares decrypted values: an identical key is not a change.
    let key_changed = private_key != stored;
    if key_changed {
        new.insert("private_key".into(), json!("<changed>"));
    }

    let changed = changed_fields(&row, &new, &[], KEY_COLUMNS);
    if !changed.is_empty() {
        let cipher = if key_changed {
            match crate::crypto::encrypt_string(&private_key, &app_key) {
                Some(c) => c,
                None => return forward(state, parts, bytes).await,
            }
        } else {
            row.get("private_key")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let updated = sqlx::query(&format!(
            "UPDATE private_keys SET name = $1, description = $2, private_key = $3, fingerprint = $4, \
             updated_at = {NOW} WHERE id = $5"
        ))
        .bind(new.get("name").and_then(Value::as_str))
        .bind(new.get("description").and_then(Value::as_str))
        .bind(&cipher)
        .bind(&fingerprint)
        .bind(id)
        .execute(pool)
        .await;
        if let Err(e) = updated {
            return query_err("update_key", e);
        }
        let changes = audit_changes(Some(&row), Some(&new), &changed, &["private_key"]);
        let name = new.get("name").and_then(Value::as_str).map(str::to_string);
        record_mutation(
            &state,
            pool,
            &ctx,
            team_id,
            "private_key",
            "updated",
            Some(&uuid),
            name.as_deref(),
            changed,
            changes,
        )
        .await;
    }
    let name = new.get("name").cloned().unwrap_or(Value::Null);
    let request_fields: Vec<&str> = ALLOWED
        .iter()
        .copied()
        .filter(|f| input.contains_key(*f))
        .collect();
    audit::record(
        pool,
        &ctx,
        "api.private_key.updated",
        json!({"team_id": team_id, "private_key_uuid": uuid, "private_key_name": name, "changed_fields": request_fields}),
        "info",
    )
    .await;
    (StatusCode::CREATED, Json(json!({ "uuid": uuid }))).into_response()
}

/// DELETE /api/v1/security/keys/:uuid
pub async fn delete_key(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let row = match fetch_key_row(pool, team_id, &uuid).await {
        Ok(Some(r)) => as_map(r),
        Ok(None) => return not_found("Private Key not found."),
        Err(e) => return query_err("delete_key", e),
    };
    let id = row.get("id").and_then(Value::as_i64).unwrap_or_default();
    // `PrivateKey::isInUse()`.
    let in_use = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM servers WHERE private_key_id = $1) \
         OR EXISTS (SELECT 1 FROM s3_storages WHERE private_key_id = $1) \
         OR EXISTS (SELECT 1 FROM applications WHERE private_key_id = $1) \
         OR EXISTS (SELECT 1 FROM github_apps WHERE private_key_id = $1) \
         OR EXISTS (SELECT 1 FROM gitlab_apps WHERE private_key_id = $1) \
         OR EXISTS (SELECT 1 FROM gitea_apps WHERE private_key_id = $1)",
    )
    .bind(id)
    .fetch_one(pool)
    .await;
    match in_use {
        Ok(true) => {
            return unprocessable(json!({
                "message": "Private Key is in use and cannot be deleted.",
                "details": "This private key is currently being used by servers, applications, or Git integrations.",
            }))
        }
        Ok(false) => {}
        Err(e) => return query_err("delete_key", e),
    }
    if let Err(e) = sqlx::query("DELETE FROM private_keys WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
    {
        return query_err("delete_key", e);
    }
    // `deleteFromStorage`: the key file the Rust SSH layer wrote for this key.
    // ponytail: Laravel's own copy (storage/app/ssh/keys) is removed by Laravel only; it goes with Laravel.
    crate::ssh::remote::remove_key_file(&uuid);

    let name = row.get("name").and_then(Value::as_str).map(str::to_string);
    let fields: Vec<String> = KEY_COLUMNS.iter().map(|c| c.to_string()).collect();
    let changes = audit_changes(Some(&row), None, &fields, &["private_key"]);
    record_mutation(
        &state,
        pool,
        &ctx,
        team_id,
        "private_key",
        "deleted",
        Some(&uuid),
        name.as_deref(),
        vec![],
        changes,
    )
    .await;
    audit::record(
        pool,
        &ctx,
        "api.private_key.deleted",
        json!({"team_id": team_id, "private_key_uuid": uuid, "private_key_name": name}),
        "info",
    )
    .await;
    ok_json(json!({"message": "Private Key deleted."}))
}

// =====================================================================================================
// Integration tokens (read + delete)
// =====================================================================================================

/// Row fields `tokenData()` needs (never the token itself).
const TOKEN_FIELDS: &str = "jsonb_build_object('uuid', t.uuid, 'name', t.name, 'provider', t.provider, \
    'capabilities', COALESCE(t.capabilities::jsonb, '[]'::jsonb), 'metadata', t.metadata::jsonb, \
    'dns_zones_count', (SELECT count(*) FROM dns_provider_zones z WHERE z.integration_token_id = t.id), \
    'created_at', t.created_at, 'updated_at', t.updated_at)";

/// `tokenData()`: safe fields only (the secret and the provider settings are left out).
fn token_data(row: Value) -> Value {
    let m = as_map(row);
    let provider = m
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut out = Map::new();
    for k in ["uuid", "name", "provider", "capabilities"] {
        out.insert(k.into(), m.get(k).cloned().unwrap_or(Value::Null));
    }
    if provider == "cloudflare" {
        let metadata = m.get("metadata").cloned().unwrap_or(Value::Null);
        out.insert(
            "automatic_dns".into(),
            json!(metadata.get("automatic_dns") != Some(&json!(false))),
        );
        out.insert(
            "proxied_dns".into(),
            json!(metadata.get("proxied_dns") == Some(&json!(true))),
        );
        out.insert(
            "dns_zones_count".into(),
            m.get("dns_zones_count").cloned().unwrap_or(json!(0)),
        );
        out.insert(
            "zones_synced_at".into(),
            metadata
                .get("zones_synced_at")
                .cloned()
                .unwrap_or(Value::Null),
        );
    }
    for k in ["created_at", "updated_at"] {
        let v = m
            .get(k)
            .and_then(Value::as_str)
            .map(|s| json!(super::model::carbon(s)))
            .unwrap_or(Value::Null);
        out.insert(k.into(), v);
    }
    Value::Object(out)
}

/// GET /api/v1/security/integration-tokens — `latest()`.
pub async fn list_integration_tokens(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query_scalar::<_, Value>(&format!(
        "SELECT {TOKEN_FIELDS} FROM integration_tokens t WHERE t.team_id = $1 ORDER BY t.created_at DESC"
    ))
    .bind(team_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => ok_json(rows.into_iter().map(token_data).collect::<Vec<_>>()),
        Err(e) => query_err("list_integration_tokens", e),
    }
}

/// GET /api/v1/security/integration-tokens/:uuid
pub async fn get_integration_token(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "read").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match sqlx::query_scalar::<_, Value>(&format!(
        "SELECT {TOKEN_FIELDS} FROM integration_tokens t WHERE t.team_id = $1 AND t.uuid = $2"
    ))
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(row)) => ok_json(token_data(row)),
        Ok(None) => not_found("Integration token not found."),
        Err(e) => query_err("get_integration_token", e),
    }
}

/// DELETE /api/v1/security/integration-tokens/:uuid — 409 while links or managed records use it.
pub async fn delete_integration_token(
    State(state): State<crate::AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(uuid): Path<String>,
) -> Response {
    let (pool, team_id) = match ctx.require(&state, "delete").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let row = match sqlx::query(
        "SELECT id, name, provider FROM integration_tokens WHERE team_id = $1 AND uuid = $2",
    )
    .bind(team_id)
    .bind(&uuid)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return not_found("Integration token not found."),
        Err(e) => return query_err("delete_integration_token", e),
    };
    let id: i64 = row.try_get("id").unwrap_or_default();
    let name: Option<String> = row.try_get("name").ok();
    let provider: Option<String> = row.try_get("provider").ok();
    // `deletionBlocker()`.
    let blockers = sqlx::query_as::<_, (bool, bool)>(
        "SELECT EXISTS (SELECT 1 FROM secret_manager_links WHERE integration_token_id = $1), \
                EXISTS (SELECT 1 FROM managed_dns_records WHERE integration_token_id = $1)",
    )
    .bind(id)
    .fetch_one(pool)
    .await;
    let message = match blockers {
        Ok((true, _)) => Some("This token is used by one or more resources as a secret manager source. Remove those links first."),
        Ok((false, true)) => Some("This token manages DNS records. Remove those domains or records first."),
        Ok(_) => None,
        Err(e) => return query_err("delete_integration_token", e),
    };
    if let Some(message) = message {
        return (StatusCode::CONFLICT, Json(json!({ "message": message }))).into_response();
    }
    if let Err(e) = sqlx::query("DELETE FROM integration_tokens WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
    {
        return query_err("delete_integration_token", e);
    }
    audit::record(
        pool,
        &ctx,
        "api.integration_token.deleted",
        json!({"team_id": team_id, "integration_token_uuid": uuid, "integration_token_name": name, "provider": provider}),
        "info",
    )
    .await;
    ok_json(json!({"message": "Integration token deleted."}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_line_gives_phpseclib_public_key_and_fingerprint() {
        // Throwaway test key blob (generated for this test only).
        let b64 = "AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl";
        let info = parse_public_line(&format!("ssh-ed25519 {b64} comment")).unwrap();
        assert_eq!(info.public_key, format!("ssh-ed25519 {b64} "));
        let blob = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        let full = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(&blob));
        assert_eq!(info.fingerprint, full[..43]);
        assert!(info.rsa_bits.is_none());
    }

    #[test]
    fn rsa_bits_from_blob() {
        let mut n = vec![0u8];
        n.extend(vec![0xffu8; 256]);
        let mut blob = Vec::new();
        for field in [b"ssh-rsa".as_slice(), &[1, 0, 1], &n] {
            blob.extend((field.len() as u32).to_be_bytes());
            blob.extend(field);
        }
        assert_eq!(rsa_modulus_bits(&blob), Some(2048));
    }

    #[test]
    fn submitted_key_decoding() {
        assert_eq!(decode_submitted_key("-----BEGIN x"), "-----BEGIN x");
        let b64 = base64::engine::general_purpose::STANDARD.encode("-----BEGIN y");
        assert_eq!(decode_submitted_key(&b64), "-----BEGIN y");
        assert_eq!(decode_submitted_key("not base64!"), "not base64!");
    }
}
