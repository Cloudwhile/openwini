-- 访问统计：匿名指纹（sha256(密钥 || 日切 || ip || ua)），按日轮转，不存原始 IP。
-- UV = 指定窗口内 DISTINCT visitor_hash；PV = SUM(hits)。
CREATE TABLE IF NOT EXISTS visit_stats (
  day DATE NOT NULL,
  visitor_hash TEXT NOT NULL,
  kind TEXT NOT NULL,              -- 'site' | 'plugin:<plugin_id>' | 'developer:<developer_id>'
  hits INTEGER NOT NULL DEFAULT 0,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (day, visitor_hash, kind)
);
CREATE INDEX IF NOT EXISTS visit_stats_day_kind_idx ON visit_stats (day, kind);
