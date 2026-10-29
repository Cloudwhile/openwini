-- 空值使用部署配置 APP_BASE_URL；仅清除历史占位地址，保留管理员设置的地址。
UPDATE site_configs
SET value_encrypted = '', updated_at = now()
WHERE key = 'catalog_base_url'
  AND rtrim(btrim(value_encrypted), '/') = 'https://plugins.windowsisland.example';
