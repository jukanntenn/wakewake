-- PG 大数据量压力附加检查（load.md §8.3）。
-- seed 灌 10 万行后，PG 承载 10 万行 + 5k 并发 SSE。压测后跑 PG 健康检查。

-- pairing_code 查询延迟（10 万行，perf-est §6.1 实测 29µs 热缓存）
EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT)
SELECT * FROM agents WHERE pairing_code = 'seed000001aa';

-- agents 表索引健康（bloat < 20%，perf-est §10.3）
SELECT schemaname, tablename, attname, n_distinct, correlation
FROM pg_stats
WHERE tablename = 'agents';

-- PG 连接状态（active < 20，perf-est §2.2）
SELECT state, count(*)
FROM pg_stat_activity
GROUP BY state;

-- agents 行数（验证 seed 灌满 10 万）
SELECT count(*) AS agent_total
FROM agents;

-- users 行数
SELECT count(*) AS user_total
FROM users
WHERE email LIKE 'seed-%@load.wakewake.local';

-- pairing_code 唯一性（无重复）
SELECT count(*) - count(DISTINCT pairing_code) AS dup_codes
FROM agents;
