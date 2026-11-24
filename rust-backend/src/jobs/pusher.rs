//! Pusher HTTP API client for Reverb (`config/broadcasting.php` `reverb` connection): server-side
//! broadcasts of the same channel/event names Laravel's `ShouldBroadcast` events use.

use hmac::{Hmac, Mac};
use md5::{Digest, Md5};
use sha2::Sha256;

pub struct Pusher {
    base: String,
    app_id: String,
    key: String,
    secret: String,
    http: reqwest::Client,
}

impl Pusher {
    /// `PUSHER_BACKEND_{HOST,PORT,SCHEME}` and `PUSHER_APP_{ID,KEY,SECRET}`, with Laravel's defaults.
    pub fn from_env() -> Self {
        let var = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        Self {
            base: format!(
                "{}://{}:{}",
                var("PUSHER_BACKEND_SCHEME", "http"),
                var("PUSHER_BACKEND_HOST", "127.0.0.1"),
                var("PUSHER_BACKEND_PORT", "6001")
            ),
            app_id: var("PUSHER_APP_ID", "tinyhelm"),
            key: var("PUSHER_APP_KEY", "tinyhelm"),
            secret: var("PUSHER_APP_SECRET", "tinyhelm"),
            http: reqwest::Client::new(),
        }
    }

    /// Trigger `event` on `channels` (e.g. `private-team.1`) with a JSON payload.
    pub async fn trigger(
        &self,
        channels: &[&str],
        event: &str,
        data: &serde_json::Value,
    ) -> Result<(), String> {
        let body =
            serde_json::json!({ "name": event, "channels": channels, "data": data.to_string() })
                .to_string();
        let path = format!("/apps/{}/events", self.app_id);
        let query = signed_query(
            &self.key,
            &self.secret,
            "POST",
            &path,
            &body,
            chrono::Utc::now().timestamp(),
        );
        let res = self
            .http
            .post(format!("{}{path}?{query}", self.base))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| format!("broadcast failed: {e}"))?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(format!("broadcast failed: HTTP {}", res.status()))
        }
    }
}

/// Query string with `auth_signature` = HMAC-SHA256(secret, "METHOD\nPATH\nsorted query").
fn signed_query(
    key: &str,
    secret: &str,
    method: &str,
    path: &str,
    body: &str,
    timestamp: i64,
) -> String {
    let body_md5 = hex::encode(Md5::digest(body.as_bytes()));
    let query =
        format!("auth_key={key}&auth_timestamp={timestamp}&auth_version=1.0&body_md5={body_md5}");
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(format!("{method}\n{path}\n{query}").as_bytes());
    format!(
        "{query}&auth_signature={}",
        hex::encode(mac.finalize().into_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The worked example from the Pusher HTTP API reference ("Generating authentication signatures").
    #[test]
    fn signature_matches_pusher_reference_example() {
        let body = r#"{"name":"foo","channels":["project-3"],"data":"{\"some\":\"data\"}"}"#;
        let q = signed_query(
            "278d425bdf160c739803",
            "7ad3773142a6692b25b8",
            "POST",
            "/apps/3/events",
            body,
            1353088179,
        );
        assert!(
            q.contains("body_md5=ec365a775a4cd0599faeb73354201b6f"),
            "{q}"
        );
        assert!(
            q.ends_with(
                "auth_signature=da454824c97ba181a32ccc17a72625ba02771f50b50e1e7430e47a1f3f457e6c"
            ),
            "{q}"
        );
    }
}
