-- 开发者自助管理：暂停（可自行恢复）与下架（重新上架需审核）

ALTER TABLE plugins DROP CONSTRAINT plugins_status_check;
ALTER TABLE plugins ADD CONSTRAINT plugins_status_check
    CHECK (status IN ('draft','pending','published','rejected','cancelled','stopped','paused','unlisted'));
