use anyhow::Context;
use serde::Serialize;

use super::TtsEngine;

pub struct ApiTts {
    client: reqwest::Client,
    url: String,
    model: String,
    voice: String,
}

impl ApiTts {
    pub fn new(base_url: String, api_key: String, model: String, voice: String) -> Self {
        let client = reqwest::Client::builder()
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert(
                    reqwest::header::AUTHORIZATION,
                    format!("Bearer {api_key}").parse().expect("valid header"),
                );
                h
            })
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            url: format!("{}/audio/speech", base_url.trim_end_matches('/')),
            model,
            voice,
        }
    }
}

#[derive(Serialize)]
struct SpeechRequest<'a> {
    model: &'a str,
    input: &'a str,
    voice: &'a str,
    response_format: &'a str,
}

#[async_trait::async_trait]
impl TtsEngine for ApiTts {
    fn name(&self) -> &'static str {
        "api-tts"
    }

    async fn synthesize(&self, text: &str) -> anyhow::Result<Vec<u8>> {
        let resp = self
            .client
            .post(&self.url)
            .json(&SpeechRequest {
                model: &self.model,
                input: text,
                voice: &self.voice,
                response_format: "wav",
            })
            .send()
            .await
            .context("TTS request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let body: String = body.chars().take(300).collect();
            anyhow::bail!("TTS API {status}: {body}");
        }
        Ok(resp
            .bytes()
            .await
            .context("failed to read TTS response")?
            .to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn posts_to_audio_speech() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/audio/speech"))
            .and(header("authorization", "Bearer k"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"RIFFfakewav".to_vec()))
            .mount(&server)
            .await;
        let e = ApiTts::new(server.uri(), "k".into(), "tts-1".into(), "alloy".into());
        let bytes = e.synthesize("xin chào").await.unwrap();
        assert_eq!(bytes, b"RIFFfakewav");
    }

    #[tokio::test]
    async fn api_error_surfaces_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(429).set_body_string("quota"))
            .mount(&server)
            .await;
        let e = ApiTts::new(server.uri(), "k".into(), "tts-1".into(), "alloy".into());
        let err = e.synthesize("x").await.unwrap_err().to_string();
        assert!(err.contains("429") || err.contains("quota"));
    }
}
