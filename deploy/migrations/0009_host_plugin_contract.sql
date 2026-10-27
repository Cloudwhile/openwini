-- Seed host capabilities while retaining developer-defined permission labels.
INSERT INTO known_permissions (name, risk, description) VALUES
    ('notifications', 'low', '向宿主提交通知'),
    ('settings', 'low', '插件独立设置与数据目录'),
    ('wechat', 'high', '默认源微信插件的宿主监听服务'),
    ('network', 'high', '通过宿主代理访问已披露的 HTTPS 目的地'),
    ('process', 'high', '进程能力声明，外部插件不获扩权'),
    ('filesystem', 'high', '任意文件能力声明，外部插件不获扩权'),
    ('native', 'high', '原生调用声明，外部插件不获扩权'),
    ('automation', 'high', '自动化能力声明，外部插件不获扩权')
ON CONFLICT (name) DO UPDATE SET description = EXCLUDED.description;

-- Package metadata is authoritative; the directory uses a distinct wire field name.
UPDATE submissions SET min_host_version = COALESCE(manifest_json->>'minimumHostVersion', '0.0.0')
WHERE jsonb_typeof(manifest_json->'minimumHostVersion') = 'string'
   OR (NOT manifest_json ? 'minHostVersion' AND
       (NOT manifest_json ? 'minimumHostVersion' OR manifest_json->'minimumHostVersion' = 'null'::jsonb));
