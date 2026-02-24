use std::cmp::max;
use std::time::Duration;

use crate::str::{ParseBytes, ParseDuration};

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
	std::env::var(key).ok().and_then(|s| s.parse::<T>().ok()).unwrap_or(default)
}

fn env_parse_bytes<T: TryFrom<u128>>(key: &str, default: T) -> T {
	std::env::var(key).ok().and_then(|s| s.as_str().parse_bytes::<T>().ok()).unwrap_or(default)
}

fn env_parse_duration<T: TryFrom<u128>>(key: &str, default: T) -> T {
	std::env::var(key).ok().and_then(|s| s.as_str().parse_duration::<T>().ok()).unwrap_or(default)
}

fn system_memory() -> u64 {
	let mut system = sysinfo::System::new_all();
	system.refresh_memory();
	match system.cgroup_limits() {
		Some(limits) => limits.total_memory,
		None => system.total_memory(),
	}
}

#[derive(Debug, Clone)]
pub(super) struct RocksDbConfig {
	// --------------------------------------------------
	// Basic options
	// --------------------------------------------------
	/// The number of threads to start for flushing and compaction (default: number
	/// of CPUs)
	pub(super) thread_count: usize,
	/// The maximum number of threads to use for flushing and compaction (default:
	/// number of CPUs * 2)
	pub(super) jobs_count: usize,
	/// The maximum number of open files which can be opened by RocksDB (default:
	/// 1024)
	pub(super) max_open_files: usize,
	/// The size of each uncompressed data block in bytes (default: 64 KiB)
	pub(super) block_size: usize,
	/// The write-ahead-log size limit in MiB (default: 0)
	pub(super) wal_size_limit: u64,
	/// The target file size for compaction in bytes (default: 64 MiB)
	pub(super) target_file_size_base: u64,
	/// The target file size multiplier for each compaction level (default: 2)
	pub(super) target_file_size_multiplier: usize,
	/// The number of files needed to trigger level 0 compaction (default: 4)
	pub(super) file_compaction_trigger: usize,
	/// The readahead buffer size used during compaction
	/// (default: dynamic from 4 MiB to 16 MiB)
	pub(super) compaction_readahead_size: usize,
	/// The maximum number threads which will perform compactions (default: 4)
	pub(super) max_concurrent_subcompactions: u32,
	/// Use separate queues for WAL writes and memtable writes (default: true)
	pub(super) enable_pipelined_writes: bool,
	/// The maximum number of information log files to keep (default: 10)
	pub(super) keep_log_file_num: usize,
	/// The information log level of the RocksDB library (default: "warn")
	pub(super) storage_log_level: String,
	/// Use to specify the database compaction style (default: "level")
	pub(super) compaction_style: String,
	/// The size of the window used to track deletions (default: 1000)
	pub(super) deletion_factory_window_size: usize,
	/// The number of deletions to track in the window (default: 50)
	pub(super) deletion_factory_delete_count: usize,
	/// The ratio of deletions to track in the window (default: 0.5)
	pub(super) deletion_factory_ratio: f64,

	// --------------------------------------------------
	// Blob file options
	// --------------------------------------------------
	/// Whether to enable separate key and value file storage (default: true)
	pub(super) enable_blob_files: bool,
	/// The minimum size of a value for it to be stored in blob files (default: 4
	/// KiB)
	pub(super) min_blob_size: u64,
	/// The target blob file size (default: 256 MiB)
	pub(super) blob_file_size: u64,
	/// Compression type used for blob files (default: "snappy")
	/// Supported values: "none", "snappy", "lz4", "zstd"
	pub(super) blob_compression_type: Option<String>,
	/// Whether to enable blob garbage collection (default: true)
	pub(super) enable_blob_gc: bool,
	/// Fractional age cutoff for blob GC eligibility between 0 and 1 (default: 0.5)
	pub(super) blob_gc_age_cutoff: f64,
	/// Discardable ratio threshold to force GC between 0 and 1 (default: 0.5)
	pub(super) blob_gc_force_threshold: f64,
	/// Readahead size for blob compaction/GC (default: 0)
	pub(super) blob_compaction_readahead_size: u64,

	// --------------------------------------------------
	// Memory manager options
	// --------------------------------------------------
	/// The size of the least-recently-used block cache
	/// (default: dynamic depending on system memory)
	pub(super) block_cache_size: usize,
	/// The amount of data each write buffer can build up in memory
	/// (default: dynamic from 32 MiB to 128 MiB)
	pub(super) write_buffer_size: usize,
	/// The maximum number of write buffers which can be used
	/// (default: dynamic from 2 to 32)
	pub(super) max_write_buffer_number: usize,
	/// The minimum number of write buffers to merge before writing to disk
	/// (default: 2)
	pub(super) min_write_buffer_number_to_merge: usize,

	// --------------------------------------------------
	// Disk space manager options
	// --------------------------------------------------
	/// The maximum allowed space usage for SST files in bytes (default: 0, meaning unlimited).
	/// When this limit is reached, the datastore enters read-and-deletion-only mode, where only
	/// read and delete operations are allowed. This allows gradual space recovery through data
	/// deletion. Set to 0 to disable space monitoring.
	pub(super) sst_max_allowed_space_usage: u64,

	// --------------------------------------------------
	// Commit coordinator options
	// --------------------------------------------------
	/// The maximum wait time in nanoseconds before forcing a grouped commit (default: 5ms).
	/// This timeout ensures that transactions don't wait indefinitely under low concurrency and
	/// balances commit latency against write throughput.
	pub(super) grouped_commit_timeout: u64,
	/// Threshold for deciding whether to wait for more transactions (default: 12)
	/// If the current batch size is greater or equal to this threshold (and below
	/// ROCKSDB_GROUPED_COMMIT_MAX_BATCH_SIZE), then the coordinator will wait up to
	/// ROCKSDB_GROUPED_COMMIT_TIMEOUT to collect more transactions. Smaller batches are flushed
	/// immediately to preserve low latency.
	pub(super) grouped_commit_wait_threshold: usize,
	/// The maximum number of transactions in a single grouped commit batch (default: 4096)
	/// This prevents unbounded memory growth while still allowing large batches for efficiency.
	/// Larger batches improve throughput but increase memory usage and commit latency.
	pub(super) grouped_commit_max_batch_size: usize,
}

impl RocksDbConfig {
	pub(super) fn from_env() -> Self {
		let memory = system_memory();
		Self {
			// Basic options
			thread_count: env_parse("SURREAL_ROCKSDB_THREAD_COUNT", num_cpus::get()),
			jobs_count: env_parse("SURREAL_ROCKSDB_JOBS_COUNT", num_cpus::get() * 2),
			max_open_files: env_parse("SURREAL_ROCKSDB_MAX_OPEN_FILES", 1024),
			block_size: env_parse_bytes("SURREAL_ROCKSDB_BLOCK_SIZE", 64 * 1024),
			wal_size_limit: env_parse("SURREAL_ROCKSDB_WAL_SIZE_LIMIT", 0),
			target_file_size_base: env_parse_bytes(
				"SURREAL_ROCKSDB_TARGET_FILE_SIZE_BASE",
				64 * 1024 * 1024,
			),
			target_file_size_multiplier: env_parse(
				"SURREAL_ROCKSDB_TARGET_FILE_SIZE_MULTIPLIER",
				2,
			),
			file_compaction_trigger: env_parse("SURREAL_ROCKSDB_FILE_COMPACTION_TRIGGER", 4),
			compaction_readahead_size: env_parse_bytes(
				"SURREAL_ROCKSDB_COMPACTION_READAHEAD_SIZE",
				default_compaction_readahead_size(memory),
			),
			max_concurrent_subcompactions: env_parse(
				"SURREAL_ROCKSDB_MAX_CONCURRENT_SUBCOMPACTIONS",
				4,
			),
			enable_pipelined_writes: env_parse("SURREAL_ROCKSDB_ENABLE_PIPELINED_WRITES", true),
			keep_log_file_num: env_parse("SURREAL_ROCKSDB_KEEP_LOG_FILE_NUM", 10),
			storage_log_level: env_parse("SURREAL_ROCKSDB_STORAGE_LOG_LEVEL", "warn".to_string()),
			compaction_style: env_parse("SURREAL_ROCKSDB_COMPACTION_STYLE", "level".to_string()),
			deletion_factory_window_size: env_parse(
				"SURREAL_ROCKSDB_DELETION_FACTORY_WINDOW_SIZE",
				1000,
			),
			deletion_factory_delete_count: env_parse(
				"SURREAL_ROCKSDB_DELETION_FACTORY_DELETE_COUNT",
				50,
			),
			deletion_factory_ratio: env_parse("SURREAL_ROCKSDB_DELETION_FACTORY_RATIO", 0.5),
			// Blob file options
			enable_blob_files: env_parse("SURREAL_ROCKSDB_ENABLE_BLOB_FILES", true),
			min_blob_size: env_parse_bytes("SURREAL_ROCKSDB_MIN_BLOB_SIZE", 4 * 1024),
			blob_file_size: env_parse_bytes("SURREAL_ROCKSDB_BLOB_FILE_SIZE", 256 * 1024 * 1024),
			blob_compression_type: std::env::var("SURREAL_ROCKSDB_BLOB_COMPRESSION_TYPE").ok(),
			enable_blob_gc: env_parse("SURREAL_ROCKSDB_ENABLE_BLOB_GC", true),
			blob_gc_age_cutoff: env_parse("SURREAL_ROCKSDB_BLOB_GC_AGE_CUTOFF", 0.5),
			blob_gc_force_threshold: env_parse("SURREAL_ROCKSDB_BLOB_GC_FORCE_THRESHOLD", 0.5),
			blob_compaction_readahead_size: env_parse_bytes(
				"SURREAL_ROCKSDB_BLOB_COMPACTION_READAHEAD_SIZE",
				0,
			),
			// Memory manager options
			block_cache_size: env_parse_bytes(
				"SURREAL_ROCKSDB_BLOCK_CACHE_SIZE",
				default_block_cache_size(memory),
			),
			write_buffer_size: env_parse_bytes(
				"SURREAL_ROCKSDB_WRITE_BUFFER_SIZE",
				default_write_buffer_size(memory),
			),
			max_write_buffer_number: env_parse(
				"SURREAL_ROCKSDB_MAX_WRITE_BUFFER_NUMBER",
				default_max_write_buffer_number(memory),
			),
			min_write_buffer_number_to_merge: env_parse(
				"SURREAL_ROCKSDB_MIN_WRITE_BUFFER_NUMBER_TO_MERGE",
				2,
			),
			// Disk space manager options
			sst_max_allowed_space_usage: env_parse_bytes(
				"SURREAL_ROCKSDB_SST_MAX_ALLOWED_SPACE_USAGE",
				0,
			),
			// Commit coordinator options
			grouped_commit_timeout: env_parse_duration(
				"SURREAL_ROCKSDB_GROUPED_COMMIT_TIMEOUT",
				Duration::from_millis(5).as_nanos() as u64,
			),
			grouped_commit_wait_threshold: env_parse(
				"SURREAL_ROCKSDB_GROUPED_COMMIT_WAIT_THRESHOLD",
				12,
			),
			grouped_commit_max_batch_size: env_parse(
				"SURREAL_ROCKSDB_GROUPED_COMMIT_MAX_BATCH_SIZE",
				4096,
			),
		}
	}
}

fn default_compaction_readahead_size(memory: u64) -> usize {
	if memory < 4 * 1024 * 1024 * 1024 {
		4 * 1024 * 1024 // For systems with < 4 GiB, use 4 MiB
	} else if memory < 16 * 1024 * 1024 * 1024 {
		8 * 1024 * 1024 // For systems with < 16 GiB, use 8 MiB
	} else {
		16 * 1024 * 1024 // For all other systems, use 16 MiB
	}
}

fn default_block_cache_size(memory: u64) -> usize {
	let memory = memory.saturating_div(2);
	let memory = memory.saturating_sub(1024 * 1024 * 1024);
	max(memory as usize, 16 * 1024 * 1024)
}

fn default_write_buffer_size(memory: u64) -> usize {
	if memory < 1024 * 1024 * 1024 {
		32 * 1024 * 1024 // For systems with < 1 GiB, use 32 MiB
	} else if memory < 16 * 1024 * 1024 * 1024 {
		64 * 1024 * 1024 // For systems with < 16 GiB, use 64 MiB
	} else {
		128 * 1024 * 1024 // For all other systems, use 128 MiB
	}
}

fn default_max_write_buffer_number(memory: u64) -> usize {
	if memory < 4 * 1024 * 1024 * 1024 {
		2 // For systems with < 4 GiB, use 2 buffers
	} else if memory < 16 * 1024 * 1024 * 1024 {
		4 // For systems with < 16 GiB, use 4 buffers
	} else if memory < 64 * 1024 * 1024 * 1024 {
		8 // For systems with < 64 GiB, use 8 buffers
	} else {
		32 // For systems with > 64 GiB, use 32 buffers
	}
}
