pub(crate) mod dynamic;

use std::path::PathBuf;
use std::sync::LazyLock;

use crate::iam::file::extract_allowed_paths;

// ---------------------------------------------------------------------------
// True constants (no env vars)
// ---------------------------------------------------------------------------

/// The publicly visible name of the server
pub const SERVER_NAME: &str = "SurrealDB";

/// The characters which are supported in server record IDs
pub const ID_CHARS: [char; 36] = [
	'0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
	'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];

/// Specifies the names of parameters which can not be specified in a query
pub const PROTECTED_PARAM_NAMES: &[&str] = &["access", "auth", "token", "session"];

// ---------------------------------------------------------------------------
// Statics that stay global (genuinely no path to receive config)
// ---------------------------------------------------------------------------

/// The memory usage threshold before tasks are forced to exit (default: 0
/// bytes). The default 0 bytes means that there is no memory threshold.
/// Any other user-set memory threshold will default to at least 1 MiB.
pub static MEMORY_THRESHOLD: LazyLock<usize> = LazyLock::new(|| {
	let n = std::env::var("SURREAL_MEMORY_THRESHOLD")
		.map(|s| s.parse::<usize>().unwrap_or(0))
		.unwrap_or(0);
	match n {
		default @ 0 => default,
		specified => std::cmp::max(specified, 1024 * 1024),
	}
});

/// Specifies the number of computed regexes which can be cached in the engine
/// (default: 1000). Kept global because it governs a process-wide thread-local
/// regex cache used from parsing, deserialization, and query execution.
pub static REGEX_CACHE_SIZE: LazyLock<usize> =
	lazy_env_parse!("SURREAL_REGEX_CACHE_SIZE", usize, 1_000);

// ---------------------------------------------------------------------------
// Env-var parsing helpers (non-lazy, used by from_env() constructors)
// ---------------------------------------------------------------------------

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
	std::env::var(key).ok().and_then(|s| s.parse::<T>().ok()).unwrap_or(default)
}

fn env_parse_with<T>(
	key: &str,
	default_fn: impl FnOnce() -> T,
	parse: impl FnOnce(String) -> Option<T>,
) -> T {
	std::env::var(key).ok().and_then(parse).unwrap_or_else(default_fn)
}

// ---------------------------------------------------------------------------
// CoreConfig – the top-level config struct loaded from env vars
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CoreConfig {
	pub limits: LimitsConfig,
	pub scripting: ScriptingConfig,
	pub http_client: HttpClientConfig,
	pub caches: CacheConfig,
	pub batching: BatchConfig,
	pub security: SecurityConfig,
	pub files: FileConfig,
}

impl CoreConfig {
	/// Build configuration from environment variables (same `SURREAL_*` names
	/// and defaults as the old `LazyLock` statics).
	pub fn from_env() -> Self {
		Self {
			limits: LimitsConfig::from_env(),
			scripting: ScriptingConfig::from_env(),
			http_client: HttpClientConfig::from_env(),
			caches: CacheConfig::from_env(),
			batching: BatchConfig::from_env(),
			security: SecurityConfig::from_env(),
			files: FileConfig::from_env(),
		}
	}
}

/// `Default` provides hard-coded defaults with *no* env-var reads, suitable
/// for unit tests.
impl Default for CoreConfig {
	fn default() -> Self {
		Self {
			limits: LimitsConfig::default(),
			scripting: ScriptingConfig::default(),
			http_client: HttpClientConfig::default(),
			caches: CacheConfig::default(),
			batching: BatchConfig::default(),
			security: SecurityConfig::default(),
			files: FileConfig::default(),
		}
	}
}

// ---------------------------------------------------------------------------
// LimitsConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LimitsConfig {
	/// Specifies how deep recursive computation will go before erroring (default: 120)
	pub max_computation_depth: u32,
	/// Specifies how deep the parser will parse nested objects and arrays (default: 100)
	pub max_object_parsing_depth: u32,
	/// Specifies how deep the parser will parse recursive queries (default: 20)
	pub max_query_parsing_depth: u32,
	/// The maximum recursive idiom path depth allowed (default: 256)
	pub idiom_recursion_limit: usize,
	/// The maximum size of a compiled regular expression (default: 10 MiB)
	pub regex_size_limit: usize,
	/// Specifies how many concurrent jobs can be buffered in the worker channel (default: 64)
	pub max_concurrent_tasks: usize,
	/// Used to limit allocation for builtin functions (default: 2^20 = 1 MiB)
	pub generation_allocation_limit: usize,
	/// The maximum input string length for similarity/distance functions (default: 16384)
	pub string_similarity_limit: usize,
	/// The threshold triggering priority-queue-based result collection (default: 1000)
	pub max_order_limit_priority_queue_size: u32,
	/// The number of batches each operator buffers ahead of downstream demand (default: 2)
	pub operator_buffer_size: usize,
	/// The number of result records which will trigger on-disk sorting (default: 50_000)
	pub external_sorting_buffer_limit: usize,
}

impl LimitsConfig {
	pub fn from_env() -> Self {
		Self {
			max_computation_depth: env_parse("SURREAL_MAX_COMPUTATION_DEPTH", 120),
			max_object_parsing_depth: env_parse("SURREAL_MAX_OBJECT_PARSING_DEPTH", 100),
			max_query_parsing_depth: env_parse("SURREAL_MAX_QUERY_PARSING_DEPTH", 20),
			idiom_recursion_limit: env_parse("SURREAL_IDIOM_RECURSION_LIMIT", 256),
			regex_size_limit: env_parse("SURREAL_REGEX_SIZE_LIMIT", 10 * 1024 * 1024),
			max_concurrent_tasks: env_parse("SURREAL_MAX_CONCURRENT_TASKS", 64),
			generation_allocation_limit: env_parse_with(
				"SURREAL_GENERATION_ALLOCATION_LIMIT",
				|| 2usize.pow(20),
				|s| {
					let n = s.parse::<u32>().ok()?;
					Some(2usize.pow(n.min(28)))
				},
			),
			string_similarity_limit: env_parse("SURREAL_STRING_SIMILARITY_LIMIT", 16384),
			max_order_limit_priority_queue_size: env_parse(
				"SURREAL_MAX_ORDER_LIMIT_PRIORITY_QUEUE_SIZE",
				1000,
			),
			operator_buffer_size: env_parse("SURREAL_OPERATOR_BUFFER_SIZE", 2),
			external_sorting_buffer_limit: env_parse(
				"SURREAL_EXTERNAL_SORTING_BUFFER_LIMIT",
				50_000,
			),
		}
	}
}

impl Default for LimitsConfig {
	fn default() -> Self {
		Self {
			max_computation_depth: 120,
			max_object_parsing_depth: 100,
			max_query_parsing_depth: 20,
			idiom_recursion_limit: 256,
			regex_size_limit: 10 * 1024 * 1024,
			max_concurrent_tasks: 64,
			generation_allocation_limit: 2usize.pow(20),
			string_similarity_limit: 16384,
			max_order_limit_priority_queue_size: 1000,
			operator_buffer_size: 2,
			external_sorting_buffer_limit: 50_000,
		}
	}
}

// ---------------------------------------------------------------------------
// ScriptingConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ScriptingConfig {
	/// The maximum stack size of the JavaScript function runtime (default: 256 KiB)
	pub max_stack_size: usize,
	/// The maximum memory limit of the JavaScript function runtime (default: 2 MiB)
	pub max_memory_limit: usize,
	/// The maximum amount of time that a JavaScript function can run in ms (default: 5000)
	pub max_time_limit: usize,
}

impl ScriptingConfig {
	pub fn from_env() -> Self {
		Self {
			max_stack_size: env_parse("SURREAL_SCRIPTING_MAX_STACK_SIZE", 256 * 1024),
			max_memory_limit: env_parse("SURREAL_SCRIPTING_MAX_MEMORY_LIMIT", 2 << 20),
			max_time_limit: env_parse("SURREAL_SCRIPTING_MAX_TIME_LIMIT", 5 * 1000),
		}
	}
}

impl Default for ScriptingConfig {
	fn default() -> Self {
		Self {
			max_stack_size: 256 * 1024,
			max_memory_limit: 2 << 20,
			max_time_limit: 5 * 1000,
		}
	}
}

// ---------------------------------------------------------------------------
// HttpClientConfig (outgoing HTTP from built-in functions)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HttpClientConfig {
	/// The maximum number of HTTP redirects allowed (default: 10)
	pub max_redirects: usize,
	/// The maximum number of idle HTTP connections to maintain per host (default: 128)
	pub max_idle_connections_per_host: usize,
	/// The maximum number of total idle HTTP connections to maintain (default: 1000)
	pub max_idle_connections: usize,
	/// The timeout for idle HTTP connections before closing in seconds (default: 90)
	pub idle_timeout_secs: u64,
	/// The timeout for connecting to HTTP endpoints in seconds (default: 30)
	pub connect_timeout_secs: u64,
	/// The USER-AGENT string used by HTTP requests (default: "SurrealDB")
	pub user_agent: String,
}

impl HttpClientConfig {
	pub fn from_env() -> Self {
		Self {
			max_redirects: env_parse("SURREAL_MAX_HTTP_REDIRECTS", 10),
			max_idle_connections_per_host: env_parse(
				"SURREAL_MAX_HTTP_IDLE_CONNECTIONS_PER_HOST",
				128,
			),
			max_idle_connections: env_parse("SURREAL_MAX_HTTP_IDLE_CONNECTIONS", 1000),
			idle_timeout_secs: env_parse("SURREAL_HTTP_IDLE_TIMEOUT_SECS", 90),
			connect_timeout_secs: env_parse("SURREAL_HTTP_CONNECT_TIMEOUT_SECS", 30),
			user_agent: std::env::var("SURREAL_USER_AGENT")
				.unwrap_or_else(|_| "SurrealDB".to_string()),
		}
	}
}

impl Default for HttpClientConfig {
	fn default() -> Self {
		Self {
			max_redirects: 10,
			max_idle_connections_per_host: 128,
			max_idle_connections: 1000,
			idle_timeout_secs: 90,
			connect_timeout_secs: 30,
			user_agent: "SurrealDB".to_string(),
		}
	}
}

// ---------------------------------------------------------------------------
// CacheConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CacheConfig {
	/// The number of items cached within a single transaction (default: 10_000)
	pub transaction_cache_size: usize,
	/// The number of definitions cached across transactions (default: 1_000)
	pub datastore_cache_size: usize,
	/// The number of surrealism modules cached across transactions (default: 100)
	pub surrealism_cache_size: usize,
	/// The maximum size of the HNSW vector cache (default: 256 MiB)
	pub hnsw_cache_size: u64,
}

impl CacheConfig {
	pub fn from_env() -> Self {
		Self {
			transaction_cache_size: env_parse("SURREAL_TRANSACTION_CACHE_SIZE", 10_000),
			datastore_cache_size: env_parse("SURREAL_DATASTORE_CACHE_SIZE", 1_000),
			surrealism_cache_size: env_parse("SURREAL_SURREALISM_CACHE_SIZE", 100),
			hnsw_cache_size: env_parse("SURREAL_HNSW_CACHE_SIZE", 256 * 1024 * 1024),
		}
	}
}

impl Default for CacheConfig {
	fn default() -> Self {
		Self {
			transaction_cache_size: 10_000,
			datastore_cache_size: 1_000,
			surrealism_cache_size: 100,
			hnsw_cache_size: 256 * 1024 * 1024,
		}
	}
}

// ---------------------------------------------------------------------------
// BatchConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BatchConfig {
	/// The maximum number of keys scanned at once in general queries (default: 500)
	pub normal_fetch_size: u32,
	/// The maximum number of keys scanned at once for export queries (default: 1000)
	pub export_batch_size: u32,
	/// The maximum number of keys scanned at once for count queries (default: 50_000)
	pub count_batch_size: u32,
	/// The maximum number of keys to scan at once per concurrent indexing batch (default: 250)
	pub indexing_batch_size: u32,
}

impl BatchConfig {
	pub fn from_env() -> Self {
		Self {
			normal_fetch_size: env_parse("SURREAL_NORMAL_FETCH_SIZE", 500),
			export_batch_size: env_parse("SURREAL_EXPORT_BATCH_SIZE", 1000),
			count_batch_size: env_parse("SURREAL_COUNT_BATCH_SIZE", 50_000),
			indexing_batch_size: env_parse("SURREAL_INDEXING_BATCH_SIZE", 250),
		}
	}
}

impl Default for BatchConfig {
	fn default() -> Self {
		Self {
			normal_fetch_size: 500,
			export_batch_size: 1000,
			count_batch_size: 50_000,
			indexing_batch_size: 250,
		}
	}
}

// ---------------------------------------------------------------------------
// SecurityConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct SecurityConfig {
	/// Forward all authentication errors to the client. Do not use in production (default: false)
	pub insecure_forward_access_errors: bool,
}

impl SecurityConfig {
	pub fn from_env() -> Self {
		Self {
			insecure_forward_access_errors: env_parse(
				"SURREAL_INSECURE_FORWARD_ACCESS_ERRORS",
				false,
			),
		}
	}
}

// ---------------------------------------------------------------------------
// FileConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct FileConfig {
	/// Paths in which files can be accessed (default: empty)
	pub file_allowlist: Vec<PathBuf>,
	/// Paths in which bucket folders can be accessed (default: empty)
	pub bucket_folder_allowlist: Vec<PathBuf>,
	/// The name of a global bucket for file data (default: None)
	pub global_bucket: Option<String>,
	/// Whether to enforce a global bucket for file data (default: false)
	pub global_bucket_enforced: bool,
}

impl FileConfig {
	pub fn from_env() -> Self {
		Self {
			file_allowlist: std::env::var("SURREAL_FILE_ALLOWLIST")
				.map(|input| extract_allowed_paths(&input, true, "file"))
				.unwrap_or_default(),
			bucket_folder_allowlist: std::env::var("SURREAL_BUCKET_FOLDER_ALLOWLIST")
				.map(|input| extract_allowed_paths(&input, false, "bucket folder"))
				.unwrap_or_default(),
			global_bucket: std::env::var("SURREAL_GLOBAL_BUCKET").ok(),
			global_bucket_enforced: env_parse("SURREAL_GLOBAL_BUCKET_ENFORCED", false),
		}
	}
}
