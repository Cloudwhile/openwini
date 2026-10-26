-- 首次登录改为自动以 GitHub 用户名派生 developerId，
-- 不再需要「待注册」中间表

DROP TABLE IF EXISTS pending_registrations;
