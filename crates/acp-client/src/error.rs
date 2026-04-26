use thiserror::Error;

#[derive(Debug, Error)]
pub enum AcpError {
    #[error("failed to spawn agent process `{command}`: {source}")]
    Spawn {
        command: String,
        #[source]
        source: std::io::Error,
    },

    #[error("agent process exited before responding")]
    ProcessExited,

    #[error("transport I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("could not encode JSON-RPC frame: {0}")]
    Encode(serde_json::Error),

    #[error("could not decode JSON-RPC frame: {0}")]
    Decode(serde_json::Error),

    #[error("agent returned a JSON-RPC error: code {code}, {message}")]
    RpcError { code: i64, message: String },

    #[error("response received for unknown request id `{0}`")]
    UnknownResponse(String),

    #[error("session was closed before completing the request")]
    Closed,

    #[error("operation timed out")]
    Timeout,
}
