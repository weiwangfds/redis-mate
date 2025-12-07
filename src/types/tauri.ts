/**
 * Standard backend response structure (aligned with Rust `CommandResponse<T>`)
 *
 * - `success`: Operation success status
 * - `code`: Response code (OK or error code)
 * - `message`: Response message (error description or empty string)
 * - `data`: Actual data payload (returned on success, null/undefined on failure)
 */
export type CommandResponse<T> = {
  success: boolean;
  code: string;
  message: string;
  data?: T | null;
};

import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

/**
 * Unified Tauri command invocation wrapper
 *
 * - Parses backend `CommandResponse<T>` into direct `T`
 * - Throws error with `code: message` when backend returns `success=false`
 *
 * @param cmd Command name (corresponds to backend `#[tauri::command]`)
 * @param payload Argument object passed to command
 */
export async function invokeCommand<T>(
  cmd: string,
  payload?: Record<string, unknown>
): Promise<T> {
  try {
    const resp = await invoke<CommandResponse<T>>(cmd, payload ?? {});
    if (resp.success) {
      return (resp.data ?? null) as T;
    }
    throw new Error(`${resp.code}: ${resp.message}`);
  } catch (e: any) {
    throw e;
  }
}

/**
 * Frontend Redis configuration object (aligned with backend `RedisConfig`)
 * 
 * Supports three modes:
 * 1. **Standalone mode**: only needs `urls` (single address)
 * 2. **Cluster mode**: set `cluster: true`, and provide seed node addresses in `urls`
 * 3. **Sentinel mode**: set `sentinel: true`, provide `sentinel_master_name` and `sentinel_urls`
 * 
 * @example
 * // Standalone
 * { urls: ["redis://127.0.0.1:6379"] }
 * 
 * // Cluster
 * { 
 *   cluster: true, 
 *   urls: ["redis://127.0.0.1:7000", "redis://127.0.0.1:7001"] 
 * }
 * 
 * // Sentinel
 * {
 *   sentinel: true,
 *   sentinel_master_name: "mymaster",
 *   sentinel_urls: ["redis://127.0.0.1:26379", "redis://127.0.0.1:26380"]
 * }
 */
export type RedisConfig = {
  /** Connection address list. Single address for standalone; seed node list for cluster */
  urls: string[];
  /** Whether to enable cluster mode */
  cluster?: boolean;
  /** Connection pool size (default 16) */
  pool_size?: number;
  /** Retry count (default 3) */
  retries?: number;
  /** Retry delay (milliseconds, default 200) */
  retry_delay_ms?: number;
  /** Whether to enable sentinel mode */
  sentinel?: boolean;
  /** Sentinel monitored master name (required for sentinel mode) */
  sentinel_master_name?: string | null;
  /** Sentinel node address list (required for sentinel mode) */
  sentinel_urls?: string[];
};

export type ConfigItem = {
  /** Configuration name (unique identifier) */
  name: string;
  /** Redis connection configuration details */
  config: RedisConfig;
};

/**
 * Add new Redis connection configuration and initialize service instance
 * 
 * This operation saves the configuration to the database and immediately attempts to establish a connection.
 * If successful, the service will be added to the in-memory service pool for subsequent use.
 * 
 * @param name Connection name (unique identifier, cannot be repeated)
 * @param config Redis connection configuration object
 * @returns true on success
 */
export async function addConnection(name: string, config: RedisConfig): Promise<boolean> {
  return invokeCommand<boolean>("add_connection", { name, config });
}

/**
 * Remove Redis connection
 * 
 * Deletes configuration from database and stops corresponding service instance from memory.
 * 
 * @param name Connection name
 * @returns true on success
 */
export async function removeConnection(name: string): Promise<boolean> {
  return invokeCommand<boolean>("remove_connection", { name });
}

/**
 * Check if connection is available (PING)
 * 
 * @param name Connection name
 * @returns true if connection is normal
 */
export async function checkConnection(name: string): Promise<boolean> {
  return invokeCommand<boolean>("check_connection", { name });
}

/**
 * Get string value (GET)
 * 
 * @param name Connection name
 * @param key Key name
 * @returns String value, or null if key does not exist
 */
export async function getValue(name: string, key: string, db?: number): Promise<string | null> {
  return invokeCommand<string | null>("get_value", { name, key, db });
}

/**
 * Set string value (SET)
 * 
 * @param name Connection name
 * @param key Key name
 * @param value String value
 * @returns true on success
 */
export async function setValue(name: string, key: string, value: string, expireSeconds?: number, db?: number): Promise<boolean> {
  return invokeCommand<boolean>("set_value", { name, key, value, expire_seconds: expireSeconds, db });
}

/**
 * Delete key (DEL)
 * 
 * @param name Connection name
 * @param key Key name
 * @returns true on success, false if key does not exist
 */
export async function delKey(name: string, key: string, db?: number): Promise<boolean> {
  return invokeCommand<boolean>("del_key", { name, key, db });
}

/**
 * Batch get values (MGET)
 * 
 * @param name Connection name
 * @param keys Array of key names
 * @returns Array of values (corresponding to index positions), null for non-existent keys
 */
export async function mgetValues(name: string, keys: string[], db?: number): Promise<(string | null)[]> {
  return invokeCommand<(string | null)[]>("mget_values", { name, keys, db });
}

/**
 * Batch set values (MSET)
 * 
 * @param name Connection name
 * @param items Array of key-value pairs [key, value][]
 * @returns true on success
 */
export async function msetValues(name: string, items: [string, string][], db?: number): Promise<boolean> {
  return invokeCommand<boolean>("mset_values", { name, items, db });
}

/**
 * Publish message (PUBLISH)
 * 
 * @param name Connection name
 * @param channel Channel name
 * @param message Message content
 * @returns Number of subscribers who received the message
 */
export async function publishMessage(name: string, channel: string, message: string): Promise<number> {
  return invokeCommand<number>("publish_message", { name, channel, message });
}

/**
 * Subscribe to channel messages
 * 
 * @param name Connection name
 * @param channel Channel name
 * @param callback Callback function when message is received
 * @returns Unsubscribe function
 */
export async function subscribeChannel(
  name: string,
  channel: string,
  event: string,
  callback: (msg: string) => void
): Promise<UnlistenFn> {
  await invokeCommand("subscribe_channel", { name, channel, event });
  return await listen<string>(event, (evt) => {
    callback(evt.payload);
  });
}

/**
 * Try to acquire distributed lock (SET NX PX)
 * 
 * @param name Connection name
 * @param resource Resource name (key)
 * @param token Lock identifier (for unlock verification)
 * @param ttl_ms Lock auto-expiration time (milliseconds)
 * @returns true on success
 */
export async function tryLock(name: string, resource: string, token: string, ttl_ms: number): Promise<boolean> {
  return invokeCommand<boolean>("try_lock", { name, resource, token, ttl_ms });
}

/**
 * Release distributed lock (Lua Script)
 * 
 * @param name Connection name
 * @param resource Resource name (key)
 * @param token Lock identifier (must match lock time)
 * @returns true on success
 */
export async function unlock(name: string, resource: string, token: string): Promise<boolean> {
  return invokeCommand<boolean>("unlock", { name, resource, token });
}

/**
 * Remove key expiration time (PERSIST)
 * 
 * @param name Connection name
 * @param key Key name
 * @returns true on success
 */
export async function persistKey(name: string, key: string, db?: number): Promise<boolean> {
  return invokeCommand<boolean>("persist_key", { name, key, db });
}

/**
 * Set key expiration time (EXPIRE)
 * 
 * @param name Connection name
 * @param key Key name
 * @param seconds Expiration time (seconds)
 * @returns true on success
 */
export async function expireKey(name: string, key: string, seconds: number, db?: number): Promise<boolean> {
  return invokeCommand<boolean>("expire_key", { name, key, seconds, db });
}

/**
 * Query key remaining time to live (TTL)
 * 
 * @param name Connection name
 * @param key Key name
 * @returns Remaining seconds (-1: permanent, -2: not exist)
 */
export async function ttlKey(name: string, key: string, db?: number): Promise<number> {
  return invokeCommand<number>("ttl_key", { name, key, db });
}

export type ClusterNodeInfo = {
  id: string;
  addr: string;
  flags: string;
  master_id: string;
  ping_sent: number;
  pong_recv: number;
  config_epoch: number;
  link_state: string;
  slots: string[];
};

/**
 * Get cluster node information
 * 
 * @param name Connection name
 * @returns Node information array
 */
export async function getClusterInfo(name: string): Promise<ClusterNodeInfo[]> {
  return invokeCommand<ClusterNodeInfo[]>("get_cluster_info", { name });
}

/**
 * Scan keys (SCAN)
 * 
 * @param name Connection name
 * @param db Database index
 * @param cursor Cursor
 * @param pattern Match pattern
 * @param count Count
 * @returns [New cursor, Key list]
 */
export async function scanKeys(name: string, db: number, cursor: number, pattern?: string, count?: number): Promise<[number, string[]]> {
  // Rust u64 fits in JS number (safe integer limit 2^53 - 1). 
  // If cursor exceeds this, we might need BigInt or string, but for Redis scan usually fine.
  // Actually Tauri handles u64 as number if it fits, or null? 
  // Let's assume number for now as typical SCAN cursors are small enough or handled.
  // Wait, invoke returns serialized JSON. u64 in serde_json is number.
  return invokeCommand<[number, string[]]>("scan_keys", { name, db, cursor, pattern, count });
}

/**
 * Get database key count (DBSIZE)
 * 
 * @param name Connection name
 * @param db Database index
 * @returns Key count
 */
export async function getDbSize(name: string, db: number): Promise<number> {
  return invokeCommand<number>("get_db_size", { name, db });
}

/**
 * List all saved configurations
 * 
 * @returns Configuration item list
 */
export async function listConfigs(): Promise<ConfigItem[]> {
  return invokeCommand<ConfigItem[]>("list_configs");
}

/**
 * Get configuration by name
 * 
 * @param name Configuration name
 * @returns Configuration object or null
 * @example
 * const cfg = await getConfig('local')
 */
export async function getConfig(name: string): Promise<RedisConfig | null> {
  return invokeCommand<RedisConfig | null>("get_config", { name });
}

/**
 * Save (add or update) configuration to database
 *
 * @param name Configuration name (unique identifier)
 * @param config Configuration object
 * @returns true on success
 * @example
 * await saveConfig('local', { urls: ['redis://127.0.0.1:6379'] })
 */
export async function saveConfig(name: string, config: RedisConfig): Promise<boolean> {
  return invokeCommand<boolean>("save_config", { name, config });
}

/**
 * Delete configuration by name
 *
 * @param name Configuration name
 * @returns true on success
 */
export async function deleteConfig(name: string): Promise<boolean> {
  return invokeCommand<boolean>("delete_config", { name });
}

/**
 * List current in-memory service connection names (AppState.services)
 *
 * @returns Name array
 */
export async function listServices(): Promise<string[]> {
  return invokeCommand<string[]>("list_services");
}

/**
 * Reload all connections from database to memory (clear then rebuild)
 *
 * @returns String 'ok' on success
 */
export async function reloadServices(): Promise<string> {
  return invokeCommand<string>("reload_services");
}

/**
 * Check if specified service exists in memory map
 *
 * @param name Service name
 * @returns true if exists
 */
export async function serviceExists(name: string): Promise<boolean> {
  return invokeCommand<boolean>("service_exists", { name });
}

/**
 * Get key type
 */
export async function getKeyType(name: string, key: string, db?: number): Promise<string> {
  return invokeCommand<string>("get_type", { name, key, db });
}

/**
 * Get all fields and values from a hash
 */
export async function hgetAll(name: string, key: string, db?: number): Promise<Record<string, string>> {
  return invokeCommand<Record<string, string>>("hgetall_hash", { name, key, db });
}

export async function hset(name: string, key: string, field: string, value: string, db?: number): Promise<boolean> {
  return invokeCommand<boolean>("hset_field", { name, key, field, value, db });
}

export async function hdel(name: string, key: string, field: string, db?: number): Promise<boolean> {
  return invokeCommand<boolean>("hdel_field", { name, key, field, db });
}

/**
 * Push value to list (left)
 */
export async function lpush(name: string, key: string, value: string, db?: number): Promise<number> {
  return invokeCommand<number>("lpush_list", { name, key, value, db });
}

/**
 * Pop value from list (right)
 */
export async function rpop(name: string, key: string, db?: number): Promise<string | null> {
  return invokeCommand<string | null>("rpop_list", { name, key, db });
}

export async function lrange(name: string, key: string, start: number, stop: number, db?: number): Promise<string[]> {
  return invokeCommand<string[]>("lrange_list", { name, key, start, stop, db });
}

/**
 * Add member to set
 */
export async function sadd(name: string, key: string, value: string, db?: number): Promise<boolean> {
  return invokeCommand<boolean>("sadd_set", { name, key, value, db });
}

/**
 * Get all members from set
 */
export async function smembers(name: string, key: string, db?: number): Promise<string[]> {
  return invokeCommand<string[]>("smembers_set", { name, key, db });
}

export async function srem(name: string, key: string, member: string, db?: number): Promise<boolean> {
  return invokeCommand<boolean>("srem_set", { name, key, member, db });
}

export async function zadd(name: string, key: string, member: string, score: number, db?: number): Promise<number> {
  return invokeCommand<number>("zadd_zset", { name, key, member, score, db });
}

export async function zrem(name: string, key: string, member: string, db?: number): Promise<boolean> {
  return invokeCommand<boolean>("zrem_zset", { name, key, member, db });
}

export async function zrangeWithScores(name: string, key: string, start: number, stop: number, db?: number): Promise<[string, number][]> {
  return invokeCommand<[string, number][]>("zrange_zset", { name, key, start, stop, db });
}

export async function jsonGet(name: string, key: string, path?: string, db?: number): Promise<any | null> {
  return invokeCommand<any | null>("json_get_value", { name, key, path, db });
}

export async function jsonSet(name: string, key: string, value: unknown, path?: string, db?: number): Promise<boolean> {
  const value_json = JSON.stringify(value);
  return invokeCommand<boolean>("json_set_value", { name, key, path, value_json, db });
}

/**
 * Test Redis connection configuration (without saving)
 * 
 * @param config Redis connection configuration
 * @returns true on success
 */
export async function testConnectionConfig(config: RedisConfig): Promise<boolean> {
  await invokeCommand<string>("test_connection_config", { config });
  return true;
}
