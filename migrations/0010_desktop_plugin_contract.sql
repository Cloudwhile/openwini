-- Desktop labels describe brokered capabilities; they never grant direct OS access.
UPDATE known_permissions SET description = '经来源核验后，通过宿主访问清单声明的客户端模块及点击目标' WHERE name = 'process';
UPDATE known_permissions SET description = '经来源及范围核验后，通过宿主读取声明客户端的可访问性与图像' WHERE name = 'automation';
UPDATE known_permissions SET description = '旧版微信适配器的历史标签，当前宿主不再授予此能力' WHERE name = 'wechat';
