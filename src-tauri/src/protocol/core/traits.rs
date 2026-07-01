// ── Protocol traits ──

use serde_json::Value;

use super::types::*;

/// Parses a wire-format request/response into the canonical representation.
pub trait InputProtocol: Send + Sync {
    fn name(&self) -> &str;
    fn parse_request(&self, body: &Value) -> anyhow::Result<ProtocolRequest>;
    fn parse_response(&self, body: &Value) -> anyhow::Result<ProtocolResponse>;
}

/// Builds wire-format request/response from the canonical representation.
pub trait OutputProtocol: Send + Sync {
    fn name(&self) -> &str;
    fn build_request(&self, req: &ProtocolRequest) -> anyhow::Result<Value>;
    fn build_response(&self, resp: &ProtocolResponse) -> anyhow::Result<Value>;
}

/// Transforms an upstream SSE byte stream into Responses API SSE events.
pub trait SseTransform: Send {
    fn push_bytes(&mut self, bytes: &[u8]) -> Vec<u8>;
    fn finish(&mut self) -> Vec<u8>;
    fn fail(&mut self, message: String, error_type: Option<String>) -> Vec<u8>;
}

/// Holds a pair of input+output protocols for a pipeline.
pub struct ProtocolPair {
    pub input: Box<dyn InputProtocol>,
    pub output: Box<dyn OutputProtocol>,
}

impl ProtocolPair {
    pub fn new(input: Box<dyn InputProtocol>, output: Box<dyn OutputProtocol>) -> Self {
        Self { input, output }
    }

    /// Convert a wire-format request through the canonical form to the output wire format.
    pub fn convert_request(&self, body: &Value) -> anyhow::Result<Value> {
        let canonical = self.input.parse_request(body)?;
        self.output.build_request(&canonical)
    }

    /// Convert a wire-format response through the canonical form to the output wire format.
    pub fn convert_response(&self, body: &Value) -> anyhow::Result<Value> {
        let canonical = self.input.parse_response(body)?;
        self.output.build_response(&canonical)
    }
}
