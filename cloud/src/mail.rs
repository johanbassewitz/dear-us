//! Outgoing email (password reset codes only). Nothing else is ever emailed.

use crate::config::Mail;
use crate::error::{ApiError, ApiResult};
use crate::AppState;
use serde_json::json;

pub async fn send(state: &AppState, to: &str, subject: &str, text: &str) -> ApiResult<()> {
    match &state.cfg.mail {
        Mail::Resend { api_key, from } => {
            let r = state
                .http
                .post("https://api.resend.com/emails")
                .bearer_auth(api_key)
                .json(&json!({ "from": from, "to": [to], "subject": subject, "text": text }))
                .send()
                .await
                .map_err(ApiError::internal)?;
            if !r.status().is_success() {
                let status = r.status();
                let body = r.text().await.unwrap_or_default();
                return Err(ApiError::internal(format!("email provider said {status}: {}", body.chars().take(200).collect::<String>())));
            }
            Ok(())
        }
        Mail::Log => {
            tracing::warn!("no email provider configured (RESEND_API_KEY): would send to {to}: {subject}\n{text}");
            Ok(())
        }
        Mail::Memory(out) => {
            out.lock().unwrap().push((to.to_string(), text.to_string()));
            Ok(())
        }
    }
}
