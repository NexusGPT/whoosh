//! The providers table: the only place an endpoint, a header scheme or a model id lives.
//! Wire shapes are written from each provider's documented JSON; the fixtures under fixtures/ are invented, not recorded
//! from the live services (docs/lanes/core-ai-providers.md "## Unverified").

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderId {
    OpenAi,
    Anthropic,
    Gemini,
    OpenRouter,
    Ollama,
    LmStudio,
    ChatGptSignIn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpeechId {
    OpenAi,
    Deepgram,
    ElevenLabs,
    Groq,
    Mistral,
}

/// How a key travels in a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    /// `Authorization: Bearer <key>`
    Bearer,
    /// `Authorization: Token <key>`
    Token,
    /// `x-api-key: <key>`
    ApiKeyHeader,
    /// `x-goog-api-key: <key>`
    GoogleKeyHeader,
    /// `xi-api-key: <key>`
    XiKeyHeader,
    None,
}

impl Auth {
    pub(crate) fn headers(self, key: Option<&str>) -> Vec<(String, String)> {
        let Some(key) = key else { return Vec::new() };
        let (name, value) = match self {
            Auth::Bearer => ("Authorization", format!("Bearer {key}")),
            Auth::Token => ("Authorization", format!("Token {key}")),
            Auth::ApiKeyHeader => ("x-api-key", key.to_string()),
            Auth::GoogleKeyHeader => ("x-goog-api-key", key.to_string()),
            Auth::XiKeyHeader => ("xi-api-key", key.to_string()),
            Auth::None => return Vec::new(),
        };
        vec![(name.to_string(), value)]
    }
}

pub struct ProviderInfo {
    pub id: ProviderId,
    pub slug: &'static str,
    pub name: &'static str,
    pub base_url: &'static str,
    pub auth: Auth,
    /// False for a local server that needs no key (its address is the owner's to change).
    pub needs_key: bool,
    /// True only where the owner signs in with a consumer account instead of pasting a key.
    pub sign_in: bool,
    pub extra_headers: &'static [(&'static str, &'static str)],
    /// Shown next to the provider in Settings.
    pub note: Option<&'static str>,
}

pub const PROVIDERS: [ProviderInfo; 7] = [
    ProviderInfo {
        id: ProviderId::OpenAi,
        slug: "openai",
        name: "OpenAI",
        base_url: "https://api.openai.com/v1",
        auth: Auth::Bearer,
        needs_key: true,
        sign_in: false,
        extra_headers: &[],
        note: None,
    },
    ProviderInfo {
        id: ProviderId::Anthropic,
        slug: "anthropic",
        name: "Anthropic",
        base_url: "https://api.anthropic.com/v1",
        auth: Auth::ApiKeyHeader,
        needs_key: true,
        sign_in: false,
        extra_headers: &[("anthropic-version", "2023-06-01")],
        note: Some("Key only."),
    },
    ProviderInfo {
        id: ProviderId::Gemini,
        slug: "gemini",
        name: "Google Gemini",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        auth: Auth::GoogleKeyHeader,
        needs_key: true,
        sign_in: false,
        extra_headers: &[],
        note: None,
    },
    ProviderInfo {
        id: ProviderId::OpenRouter,
        slug: "openrouter",
        name: "OpenRouter",
        base_url: "https://openrouter.ai/api/v1",
        auth: Auth::Bearer,
        needs_key: true,
        sign_in: false,
        extra_headers: &[],
        note: None,
    },
    ProviderInfo {
        id: ProviderId::Ollama,
        slug: "ollama",
        name: "Ollama",
        base_url: "http://localhost:11434/v1",
        auth: Auth::None,
        needs_key: false,
        sign_in: false,
        extra_headers: &[],
        note: Some("Runs on this Mac. Nothing leaves it."),
    },
    ProviderInfo {
        id: ProviderId::LmStudio,
        slug: "lmstudio",
        name: "LM Studio",
        base_url: "http://localhost:1234/v1",
        auth: Auth::Bearer,
        needs_key: false,
        sign_in: false,
        extra_headers: &[],
        note: Some("Runs on this Mac. Nothing leaves it."),
    },
    ProviderInfo {
        id: ProviderId::ChatGptSignIn,
        slug: "chatgpt",
        name: "ChatGPT sign-in",
        base_url: "https://chatgpt.com/backend-api/codex",
        auth: Auth::Bearer,
        needs_key: true,
        sign_in: true,
        extra_headers: &[],
        note: Some("Uses your ChatGPT subscription through a sign-in OpenAI publishes no policy for in other apps. It can stop working at any time."),
    },
];

pub fn info(id: ProviderId) -> &'static ProviderInfo {
    PROVIDERS.iter().find(|p| p.id == id).expect("every ProviderId has a row in PROVIDERS")
}

/// ai-settings-core: the provider and model the built-in agents (Ask my inbox, the Writer) start on before the owner picks one in
/// Settings, AI and agents: the fixture core's built-ins (Sources/CoreBridge/FixtureAgents.swift). Nothing is called until the owner
/// saves that provider's key.
pub const BUILT_IN_AGENT_PROVIDER: ProviderId = ProviderId::Anthropic;
pub const BUILT_IN_AGENT_MODEL: &str = "claude-sonnet-5-5";

pub struct SpeechInfo {
    pub id: SpeechId,
    pub slug: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub model: &'static str,
    pub auth: Auth,
}

pub const SPEECH: [SpeechInfo; 5] = [
    SpeechInfo { id: SpeechId::OpenAi, slug: "openai", name: "OpenAI transcription", url: "https://api.openai.com/v1/audio/transcriptions", model: "gpt-4o-transcribe", auth: Auth::Bearer },
    SpeechInfo { id: SpeechId::Deepgram, slug: "deepgram", name: "Deepgram", url: "https://api.deepgram.com/v1/listen", model: "nova-3", auth: Auth::Token },
    SpeechInfo { id: SpeechId::ElevenLabs, slug: "elevenlabs", name: "ElevenLabs Scribe", url: "https://api.elevenlabs.io/v1/speech-to-text", model: "scribe_v1", auth: Auth::XiKeyHeader },
    SpeechInfo { id: SpeechId::Groq, slug: "groq", name: "Groq", url: "https://api.groq.com/openai/v1/audio/transcriptions", model: "whisper-large-v3-turbo", auth: Auth::Bearer },
    SpeechInfo { id: SpeechId::Mistral, slug: "mistral", name: "Mistral Voxtral", url: "https://api.mistral.ai/v1/audio/transcriptions", model: "voxtral-mini-latest", auth: Auth::Bearer },
];

pub fn speech_info(id: SpeechId) -> &'static SpeechInfo {
    SPEECH.iter().find(|p| p.id == id).expect("every SpeechId has a row in SPEECH")
}
