use std::cmp::max;
use std::str::FromStr;

use sysinfo::System;

use crate::str::{ParseBytes, ParseDuration};

/// Configuration options for the SurrealKV storage engine.
pub(super) struct SurrealKvConfig {
	/// Whether to enable value log separation (default: true)
	pub(super) enable_vlog: bool,
	/// Whether to enable versioned index (default: false, only applies when versioning is enabled)
	pub(super) versioned_index: bool,
	/// The block size in bytes (default: 64 KiB)
	pub(super) block_size: usize,
	/// The maximum value log file size in bytes (default: dynamic from 64 MiB to 512 MiB)
	pub(super) vlog_max_file_size: u64,
	/// The value log threshold in bytes - values larger than this are stored in the value log
	/// (default: 4 KiB)
	pub(super) vlog_threshold: usize,
	/// The block cache capacity in bytes (default: dynamic based on memory)
	pub(super) block_cache_capacity: u64,
	/// The maximum wait time in nanoseconds before forcing a grouped commit (default: 5ms).
	/// This timeout ensures that transactions don't wait indefinitely under low concurrency and
	/// balances commit latency against write throughput.
	pub(super) grouped_commit_timeout: u64,
	/// Threshold for deciding whether to wait for more transactions (default: 12).
	/// If the current batch size is greater or equal to this threshold (and below
	/// `grouped_commit_max_batch_size`), then the coordinator will wait up to
	/// `grouped_commit_timeout` to collect more transactions. Smaller batches are flushed
	/// immediately to preserve low latency.
	pub(super) grouped_commit_wait_threshold: usize,
	/// The maximum number of transactions in a single grouped commit batch (default: 4096).
	/// This prevents unbounded memory growth while still allowing large batches for efficiency.
	/// Larger batches improve throughput but increase memory usage and commit latency.
	pub(super) grouped_commit_max_batch_size: usize,
}

impl SurrealKvConfig {
	pub(super) fn from_env() -> Self {
		let memory = system_memory();
		Self {
			enable_vlog: env_parse("SURREAL_SURREALKV_ENABLE_VLOG", true),
			versioned_index: env_parse("SURREAL_SURREALKV_VERSIONED_INDEX", false),
			block_size: env_parse_bytes("SURREAL_SURREALKV_BLOCK_SIZE", 64 * 1024),
			vlog_max_file_size: env_parse_bytes(
				"SURREAL_SURREALKV_VLOG_MAX_FILE_SIZE",
				default_vlog_max_file_size(memory),
			),
			vlog_threshold: env_parse_bytes("SURREAL_SURREALKV_VLOG_THRESHOLD", 4 * 1024),
			block_cache_capacity: env_parse_bytes(
				"SURREAL_SURREALKV_BLOCK_CACHE_CAPACITY",
				default_block_cache_capacity(memory),
			),
			grouped_commit_timeout: env_parse_duration(
				"SURREAL_SURREALKV_GROUPED_COMMIT_TIMEOUT",
				std::time::Duration::from_millis(5).as_nanos() as u64,
			),
			grouped_commit_wait_threshold: env_parse(
				"SURREAL_SURREALKV_GROUPED_COMMIT_WAIT_THRESHOLD",
				12,
			),
			grouped_commit_max_batch_size: env_parse(
				"SURREAL_SURREALKV_GROUPED_COMMIT_MAX_BATCH_SIZE",
				4096,
			),
		}
	}
}

fn env_parse<T: FromStr>(key: &str, default: T) -> T {
	std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_parse_bytes<T: TryFrom<u128>>(key: &str, default: T) -> T {
	std::env::var(key).ok().and_then(|v| v.as_str().parse_bytes().ok()).unwrap_or(default)
}

fn env_parse_duration<T: TryFrom<u128>>(key: &str, default: T) -> T {
	std::env::var(key).ok().and_then(|v| v.as_str().parse_duration().ok()).unwrap_or(default)
}

fn system_memory() -> u64 {
	let mut system = System::new_all();
	system.refresh_memory();
	match system.cgroup_limits() {
		Some(limits) => limits.total_memory,
		None => system.total_memory(),
	}
}

fn default_vlog_max_file_size(memory: u64) -> u64 {
	if memory < 4 * 1024 * 1024 * 1024 {
		64 * 1024 * 1024
	} else if memory < 16 * 1024 * 1024 * 1024 {
		128 * 1024 * 1024
	} else if memory < 64 * 1024 * 1024 * 1024 {
		256 * 1024 * 1024
	} else {
		512 * 1024 * 1024
	}
}

fn default_block_cache_capacity(memory: u64) -> u64 {
	let memory = memory.saturating_div(2);
	let memory = memory.saturating_sub(1024 * 1024 * 1024);
	max(memory, 16 * 1024 * 1024)
}
