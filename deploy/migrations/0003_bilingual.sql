-- 双语支持：插件中英两套描述 + 站点标题中英两套配置

ALTER TABLE submissions ADD COLUMN description_en TEXT NOT NULL DEFAULT '';

INSERT INTO site_configs (key, value_encrypted, is_secret) VALUES
    ('site_name_en', 'WindowsIsland Plugin Hub', FALSE)
  ON CONFLICT (key) DO NOTHING;
