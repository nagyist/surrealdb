use std::str::FromStr;

/// Configuration options for the TiKV storage engine.
pub(super) struct TiKvConfig {
	/// Which TiKV cluster API version to use
	pub(super) api_version: u8,
	/// The keyspace identifier for data isolation
	pub(super) keyspace: Option<String>,
	/// The duration for requests before they timeout in seconds
	pub(super) request_timeout: u64,
	/// Whether to use asynchronous transaction commit
	pub(super) async_commit: bool,
	/// Whether to use one-phase transaction commit
	pub(super) one_phase_commit: bool,
	/// Limits the maximum size of a decoded message - default value to 4MB
	pub(super) grpc_max_decoding_message_size: usize,
}

impl TiKvConfig {
	pub(super) fn from_env() -> Self {
		Self {
			api_version: env_parse("SURREAL_TIKV_API_VERSION", 1),
			keyspace: std::env::var("SURREAL_TIKV_KEYSPACE").ok(),
			request_timeout: env_parse("SURREAL_TIKV_REQUEST_TIMEOUT", 10),
			async_commit: env_parse("SURREAL_TIKV_ASYNC_COMMIT", true),
			one_phase_commit: env_parse("SURREAL_TIKV_ONE_PHASE_COMMIT", true),
			grpc_max_decoding_message_size: env_parse(
				"SURREAL_TIKV_GRPC_MAX_DECODING_MESSAGE_SIZE",
				4 * 1024 * 1024,
			),
		}
	}
}

fn env_parse<T: FromStr>(key: &str, default: T) -> T {
	std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
