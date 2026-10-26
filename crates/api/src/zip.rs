//! 插件 ZIP 包有效核验：手写 ZIP central directory 解析（无需重依赖）。
//! 核验项：PK 魔数、目录完整性、无加密条目、根目录 manifest.json 存在、
//! 包内 manifest 的 id/version 与申请一致，返回清单供调用方与仓库版本完整比对。

use std::io::Read as _;

const EOCD_SIG: u32 = 0x0605_4b50;
const CDE_SIG: u32 = 0x0201_4b50;
const LFH_SIG: u32 = 0x0403_4b50;
const ENTRY_MAX_BYTES: u32 = 8 * 1024 * 1024;

#[derive(Debug)]
pub struct ZipEntry {
    pub name: String,
    flags: u16,
    method: u16,
    crc32: u32,
    comp_size: u32,
    uncomp_size: u32,
    local_offset: u32,
}

fn u16_at(d: &[u8], off: usize) -> Option<u16> {
    d.get(off..off + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(d: &[u8], off: usize) -> Option<u32> {
    d.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// 解析 central directory 全部条目
pub fn parse_entries(data: &[u8]) -> Result<Vec<ZipEntry>, String> {
    if data.len() < 4 || &data[0..4] != b"PK\x03\x04" {
        return Err("不是有效的 ZIP 文件（缺少 PK 头），请确认附件是未加密的标准 .zip 包".into());
    }
    // 从尾部向前找 EOCD（注释最长 65535 + 22）
    let tail_start = data.len().saturating_sub(22 + 65535);
    let mut eocd_off = None;
    let mut i = data.len();
    while i >= tail_start + 4 {
        i -= 1;
        if u32_at(data, i) == Some(EOCD_SIG) {
            eocd_off = Some(i);
            break;
        }
    }
    let eocd = eocd_off
        .ok_or("ZIP 目录损坏（找不到 End of Central Directory），文件可能不完整或被截断")?;
    let comment_len = u16_at(data, eocd + 20).ok_or("ZIP 目录损坏")? as usize;
    if eocd.checked_add(22 + comment_len) != Some(data.len()) {
        return Err("ZIP 目录损坏（结束记录长度不符）".into());
    }
    let total_entries = u16_at(data, eocd + 10).ok_or("ZIP 目录损坏")? as usize;
    if u16_at(data, eocd + 4) != Some(0)
        || u16_at(data, eocd + 6) != Some(0)
        || u16_at(data, eocd + 8) != Some(total_entries as u16)
    {
        return Err("不支持分卷 ZIP 插件包".into());
    }
    let cd_size = u32_at(data, eocd + 12).ok_or("ZIP 目录损坏")? as usize;
    let cd_offset = u32_at(data, eocd + 16).ok_or("ZIP 目录损坏")? as usize;
    let cd_end = cd_offset.checked_add(cd_size).ok_or("ZIP 目录损坏")?;
    if cd_end > eocd {
        return Err("ZIP 目录损坏（central directory 越界）".into());
    }

    let mut entries = Vec::with_capacity(total_entries.min(4096));
    let mut p = cd_offset;
    for _ in 0..total_entries {
        if p.checked_add(46).is_none_or(|end| end > cd_end) {
            return Err("ZIP 目录损坏（central directory 条目越界）".into());
        }
        if u32_at(data, p) != Some(CDE_SIG) {
            return Err("ZIP 目录损坏（central directory 条目签名不符）".into());
        }
        let flags = u16_at(data, p + 8).ok_or("ZIP 目录损坏")?;
        let method = u16_at(data, p + 10).ok_or("ZIP 目录损坏")?;
        let crc32 = u32_at(data, p + 16).ok_or("ZIP 目录损坏")?;
        let comp_size = u32_at(data, p + 20).ok_or("ZIP 目录损坏")?;
        let uncomp_size = u32_at(data, p + 24).ok_or("ZIP 目录损坏")?;
        let name_len = u16_at(data, p + 28).ok_or("ZIP 目录损坏")? as usize;
        let extra_len = u16_at(data, p + 30).ok_or("ZIP 目录损坏")? as usize;
        let comment_len = u16_at(data, p + 32).ok_or("ZIP 目录损坏")? as usize;
        let local_offset = u32_at(data, p + 42).ok_or("ZIP 目录损坏")?;
        let name_start = p + 46;
        let name_bytes = data
            .get(name_start..name_start + name_len)
            .ok_or("ZIP 目录损坏（条目名越界）")?;
        let name = decode_name(name_bytes)?;
        entries.push(ZipEntry {
            name,
            flags,
            method,
            crc32,
            comp_size,
            uncomp_size,
            local_offset,
        });
        p = name_start + name_len + extra_len + comment_len;
        if p > cd_end {
            return Err("ZIP 目录损坏（条目越界）".into());
        }
    }
    if p != cd_end {
        return Err("ZIP 目录损坏（条目数量与目录长度不符）".into());
    }
    Ok(entries)
}

fn decode_name(bytes: &[u8]) -> Result<String, String> {
    // UTF-8 标志位由调用方按 flags 处理；这里优先按 UTF-8 解，失败回退 cp437 的 ASCII 子集
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => Ok(bytes
            .iter()
            .map(|&b| if b.is_ascii() { b as char } else { '_' })
            .collect()),
    }
}

/// 读取单个条目的解压后内容（支持 stored / deflate）
pub fn read_entry(data: &[u8], e: &ZipEntry) -> Result<Vec<u8>, String> {
    if e.flags & 0x1 != 0 {
        return Err(format!("ZIP 条目 {} 已加密，不接受加密的插件包", e.name));
    }
    if e.uncomp_size > ENTRY_MAX_BYTES {
        return Err(format!("ZIP 条目 {} 解压后超过 8 MiB，异常", e.name));
    }
    let off = e.local_offset as usize;
    if u32_at(data, off) != Some(LFH_SIG) {
        return Err("ZIP 数据损坏（local header 签名不符）".into());
    }
    if u16_at(data, off + 6) != Some(e.flags) || u16_at(data, off + 8) != Some(e.method) {
        return Err("ZIP 数据损坏（条目头与目录不一致）".into());
    }
    let name_len = u16_at(data, off + 26).ok_or("ZIP 数据损坏")? as usize;
    let extra_len = u16_at(data, off + 28).ok_or("ZIP 数据损坏")? as usize;
    let data_start = off + 30 + name_len + extra_len;
    let local_name = data
        .get(off + 30..off + 30 + name_len)
        .ok_or("ZIP 数据损坏")?;
    if decode_name(local_name)? != e.name {
        return Err("ZIP 数据损坏（条目名称与目录不一致）".into());
    }
    let data_end = data_start
        .checked_add(e.comp_size as usize)
        .ok_or("ZIP 数据损坏")?;
    let raw = data
        .get(data_start..data_end)
        .ok_or("ZIP 数据损坏（条目数据越界）")?;
    let out = match e.method {
        0 => {
            if raw.len() != e.uncomp_size as usize {
                return Err(format!("ZIP 条目 {} 大小与目录记录不符", e.name));
            }
            raw.to_vec()
        }
        8 => {
            let mut out = Vec::with_capacity(e.uncomp_size as usize);
            flate2::read::DeflateDecoder::new(raw)
                .take(u64::from(e.uncomp_size) + 1)
                .read_to_end(&mut out)
                .map_err(|_| format!("ZIP 条目 {} 解压失败（数据损坏）", e.name))?;
            out
        }
        m => return Err(format!("ZIP 条目 {} 使用了不支持的压缩方法（{m}）", e.name)),
    };
    if out.len() != e.uncomp_size as usize {
        return Err(format!("ZIP 条目 {} 实际解压大小与目录记录不符", e.name));
    }
    if crc32(&out) != e.crc32 {
        return Err(format!("ZIP 条目 {} 校验和不符，文件可能已损坏", e.name));
    }
    Ok(out)
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                0xedb8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
    }
    crc ^ 0xffff_ffff
}

/// 插件包整体核验：返回 Err(可读原因) 即核验不通过
pub fn verify_plugin_zip(
    data: &[u8],
    expect_id: &str,
    expect_version: &str,
) -> Result<serde_json::Value, String> {
    let entries = parse_entries(data)?;
    if entries.is_empty() {
        return Err("ZIP 包为空".into());
    }
    if entries.iter().any(|e| e.flags & 0x1 != 0) {
        return Err("ZIP 包含加密条目，不接受加密的插件包".into());
    }
    // 根目录 manifest.json
    let mut manifests = entries.iter().filter(|e| e.name == "manifest.json");
    let manifest_entry = manifests.next();
    if manifests.next().is_some() {
        return Err("ZIP 根目录只能包含一份 manifest.json".into());
    }
    let manifest_entry = match manifest_entry {
        Some(e) => e,
        None => {
            // 常见打包错误：多套了一层目录
            let nested: Vec<&String> = entries
                .iter()
                .filter(|e| e.name.ends_with("/manifest.json"))
                .map(|e| &e.name)
                .collect();
            if let Some(first) = nested.first() {
                let top = first.split('/').next().unwrap_or("");
                return Err(format!(
                    "manifest.json 位于 ZIP 的 {top}/ 子目录内，必须放在 ZIP 根目录；请进入 {top} 目录后重新打包（不要把文件夹本身压进去）"
                ));
            }
            return Err(
                "ZIP 包内没有 manifest.json：请把插件 manifest.json 与构建产物一起打包".into(),
            );
        }
    };
    if manifest_entry.uncomp_size as usize > crate::validation::MANIFEST_MAX_BYTES {
        return Err("ZIP 内 manifest.json 超过 256 KiB 上限".into());
    }
    let bytes = read_entry(data, manifest_entry)?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "ZIP 内的 manifest.json 不是合法 JSON")?;
    let id = manifest.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let mut problems: Vec<String> = Vec::new();
    if id.is_empty() {
        problems.push("ZIP 内 manifest.json 缺少 id 字段".into());
    } else if id != expect_id {
        problems.push(format!("ZIP 内 manifest.json 的 id（{id}）与申请的插件标识（{expect_id}）不一致：包可能不是当前版本构建的"));
    }
    if version.is_empty() {
        problems.push("ZIP 内 manifest.json 缺少 version 字段".into());
    } else if version != expect_version {
        problems.push(format!(
            "ZIP 内 manifest.json 的 version（{version}）与申请版本（{expect_version}）不一致：Release 附件可能未随版本更新"
        ));
    }
    if !problems.is_empty() {
        return Err(problems.join("；"));
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crc32(data: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for (i, t) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *t = c;
        }
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
        crc ^ 0xFFFF_FFFF
    }

    /// 构造 stored（无压缩）的迷你 ZIP
    fn build_stored_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        let count = files.len() as u16;
        for (name, content) in files {
            let local_offset = out.len() as u32;
            let nb = name.as_bytes();
            let crc = crc32(content);
            out.extend_from_slice(&LFH_SIG.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&(content.len() as u32).to_le_bytes());
            out.extend_from_slice(&(content.len() as u32).to_le_bytes());
            out.extend_from_slice(&(nb.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(nb);
            out.extend_from_slice(content);

            central.extend_from_slice(&CDE_SIG.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&(content.len() as u32).to_le_bytes());
            central.extend_from_slice(&(content.len() as u32).to_le_bytes());
            central.extend_from_slice(&(nb.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&local_offset.to_le_bytes());
            central.extend_from_slice(nb);
        }
        let cd_offset = out.len() as u32;
        let cd_size = central.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&EOCD_SIG.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    const GOOD_MANIFEST: &str = r#"{"id":"wini_plugin_demo","version":"1.0.0","name":"Demo"}"#;

    #[test]
    fn valid_zip_passes() {
        let z = build_stored_zip(&[
            ("manifest.json", GOOD_MANIFEST.as_bytes()),
            ("main.dll", b"MZ..."),
        ]);
        let manifest = verify_plugin_zip(&z, "wini_plugin_demo", "1.0.0").unwrap();
        assert_eq!(manifest["name"], "Demo");
        assert!(parse_entries(&z).unwrap().len() == 2);
    }

    #[test]
    fn not_a_zip_rejected() {
        assert!(verify_plugin_zip(b"not a zip at all......", "wini_plugin_demo", "1.0.0").is_err());
    }

    #[test]
    fn missing_manifest_rejected() {
        let z = build_stored_zip(&[("main.dll", b"MZ...")]);
        let err = verify_plugin_zip(&z, "wini_plugin_demo", "1.0.0").unwrap_err();
        assert!(err.contains("没有 manifest.json"), "got: {err}");
    }

    #[test]
    fn nested_manifest_gets_packaging_hint() {
        let z = build_stored_zip(&[("wini_plugin_demo/manifest.json", GOOD_MANIFEST.as_bytes())]);
        let err = verify_plugin_zip(&z, "wini_plugin_demo", "1.0.0").unwrap_err();
        assert!(err.contains("根目录"), "got: {err}");
    }

    #[test]
    fn mismatched_id_or_version_rejected() {
        let z = build_stored_zip(&[("manifest.json", GOOD_MANIFEST.as_bytes())]);
        let err = verify_plugin_zip(&z, "wini_plugin_other", "1.0.0").unwrap_err();
        assert!(err.contains("id"), "got: {err}");
        let err = verify_plugin_zip(&z, "wini_plugin_demo", "2.0.0").unwrap_err();
        assert!(err.contains("version"), "got: {err}");
    }

    #[test]
    fn truncated_zip_rejected() {
        let z = build_stored_zip(&[("manifest.json", GOOD_MANIFEST.as_bytes())]);
        assert!(parse_entries(&z[..z.len() - 10]).is_err());
    }

    #[test]
    fn reads_entry_content() {
        let z = build_stored_zip(&[("manifest.json", GOOD_MANIFEST.as_bytes())]);
        let e = &parse_entries(&z).unwrap()[0];
        assert_eq!(read_entry(&z, e).unwrap(), GOOD_MANIFEST.as_bytes());
    }

    #[test]
    fn corrupt_manifest_bytes_fail_checksum_validation() {
        let mut z = build_stored_zip(&[("manifest.json", GOOD_MANIFEST.as_bytes())]);
        let payload = z
            .windows(GOOD_MANIFEST.len())
            .position(|window| window == GOOD_MANIFEST.as_bytes())
            .unwrap();
        z[payload] ^= 1;
        let err = verify_plugin_zip(&z, "wini_plugin_demo", "1.0.0").unwrap_err();
        assert!(err.contains("校验和"), "got: {err}");
    }
}
