import { useCallback, useEffect, useState } from "react"
import { CheckCircle2, KeyRound, Pencil, Plus, Tags, XCircle } from "lucide-react"
import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import { Dialog } from "@workspace/ui/components/dialog"
import { Input } from "@workspace/ui/components/input"
import { Label } from "@workspace/ui/components/label"
import { Switch } from "@workspace/ui/components/switch"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@workspace/ui/components/table"
import { Textarea } from "@workspace/ui/components/textarea"
import { api, fmtTime, type AiConfig, type PermissionTag, type SiteConfig, type SmtpConfig, type SmtpLogItem } from "@/lib/api"
import { useRequireAdmin } from "@/lib/auth"
import { useToast } from "@/components/toast"
import { Pagination } from "@/components/pagination"
import { ErrorState, LoadingState } from "@/components/page-states"
import { SearchableSelect } from "@/components/searchable-select"
import { ConfirmDialog } from "@/components/confirm-dialog"
import { FormSection, FormField, PageContainer, PageHeader } from "@/components/page-header"
import { cn } from "@workspace/ui/lib/utils"
import { t } from "@/i18n"

type Tab = "site" | "smtp" | "ai" | "perms"

const PERMISSION_NAME_RE = /^[a-z][a-z0-9_]*$/

export function AdminSettings() {
  const { ready, isAdmin } = useRequireAdmin()
  const { toast } = useToast()
  const [tab, setTab] = useState<Tab>("site")
  const [site, setSite] = useState<SiteConfig | null>(null)
  const [smtp, setSmtp] = useState<SmtpConfig | null>(null)
  // 密钥替换：不回显已保存密钥，仅提供显式替换入口
  const [smtpPassword, setSmtpPassword] = useState("")
  const [smtpReplacing, setSmtpReplacing] = useState(false)
  const [ai, setAi] = useState<AiConfig | null>(null)
  const [aiKey, setAiKey] = useState("")
  const [aiReplacing, setAiReplacing] = useState(false)
  const [testTo, setTestTo] = useState("")
  const [logs, setLogs] = useState<{ items: SmtpLogItem[]; total: number; page: number; pageSize: number } | null>(null)
  const [logPage, setLogPage] = useState(1)
  const [busy, setBusy] = useState(false)
  // 权限标签管理
  const [perms, setPerms] = useState<PermissionTag[] | null>(null)
  const [permDialog, setPermDialog] = useState<{
    mode: "add" | "edit"
    name: string
    risk: "low" | "medium" | "high"
    description: string
  } | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<PermissionTag | null>(null)
  const [permBusy, setPermBusy] = useState(false)
  const [configError, setConfigError] = useState("")
  const [logsError, setLogsError] = useState("")

  const loadLogs = useCallback(() => {
    api
      .get<{ items: SmtpLogItem[]; total: number; page: number; pageSize: number }>(`/api/admin/config/smtp/logs?page=${logPage}`)
      .then((value) => { setLogs(value); setLogsError("") })
      .catch((e: Error) => setLogsError(e.message))
  }, [logPage])

  useEffect(() => {
    if (!isAdmin) return
    api.get<SiteConfig>("/api/admin/config/site").then(setSite).catch((e: Error) => setConfigError(e.message))
    api.get<SmtpConfig>("/api/admin/config/smtp").then(setSmtp).catch((e: Error) => setConfigError(e.message))
    api.get<AiConfig>("/api/admin/config/ai").then(setAi).catch((e: Error) => setConfigError(e.message))
  }, [isAdmin])

  useEffect(() => {
    if (isAdmin && tab === "smtp") loadLogs()
  }, [isAdmin, tab, loadLogs])

  const loadPerms = useCallback(() => {
    api
      .get<{ permissions: PermissionTag[] }>("/api/admin/permissions")
      .then((r) => setPerms(r.permissions))
      .catch((e: Error) => toast("error", e.message))
  }, [toast])

  useEffect(() => {
    if (isAdmin && tab === "perms" && perms === null) loadPerms()
  }, [isAdmin, tab, perms, loadPerms])

  const savePerm = async () => {
    if (!permDialog) return
    setPermBusy(true)
    try {
      await api.put("/api/admin/permissions", {
        name: permDialog.name.trim(),
        risk: permDialog.risk,
        description: permDialog.description,
      })
      toast("success", permDialog.mode === "add" ? t.admin.settings.perms.created : t.admin.settings.perms.updated)
      setPermDialog(null)
      loadPerms()
    } catch (e) {
      toast("error", (e as Error).message)
    } finally {
      setPermBusy(false)
    }
  }

  const doDeletePerm = async () => {
    if (!deleteTarget) return
    try {
      await api.del(`/api/admin/permissions/${encodeURIComponent(deleteTarget.name)}`)
      toast("success", t.admin.settings.perms.deleted)
      loadPerms()
    } catch (e) {
      toast("error", (e as Error).message)
    } finally {
      setDeleteTarget(null)
    }
  }

  const tabs: { key: Tab; label: string }[] = [
    { key: "site", label: t.admin.settings.tabs.site },
    { key: "smtp", label: t.admin.settings.tabs.smtp },
    { key: "ai", label: t.admin.settings.tabs.ai },
    { key: "perms", label: t.admin.settings.tabs.perms },
  ]

  if (!ready) return null
  if (!isAdmin) return <div className="text-destructive py-20 text-center text-sm">{t.errors.needAdmin}</div>

  const saveSite = async () => {
    if (!site) return
    setBusy(true)
    try {
      await api.put("/api/admin/config/site", {
        siteName: site.siteName,
        siteNameEn: site.siteNameEn,
        announcement: site.announcement,
        pageSize: Number(site.pageSize) || 20,
        publicCatalogEnabled: site.publicCatalogEnabled,
        catalogBaseUrl: site.catalogBaseUrl,
      })
      toast("success", t.admin.settings.site.saved)
    } catch (e) {
      toast("error", (e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const saveSmtp = async () => {
    if (!smtp) return
    setBusy(true)
    try {
      await api.put("/api/admin/config/smtp", {
        host: smtp.host,
        port: smtp.port,
        username: smtp.username,
        ...(smtpPassword ? { password: smtpPassword } : {}),
        encryption: smtp.encryption,
        fromAddress: smtp.fromAddress,
        fromName: smtp.fromName,
      })
      setSmtpPassword("")
      setSmtpReplacing(false)
      toast("success", t.admin.settings.smtp.saved)
    } catch (e) {
      toast("error", (e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const sendTest = async () => {
    if (!testTo.trim()) {
      toast("error", t.admin.settings.smtp.testToRequired)
      return
    }
    setBusy(true)
    try {
      await api.post("/api/admin/config/smtp/test", { to: testTo.trim() })
      toast("success", t.misc.testQueued)
      setTimeout(loadLogs, 1500)
    } catch (e) {
      toast("error", (e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const saveAi = async () => {
    if (!ai) return
    setBusy(true)
    try {
      await api.put("/api/admin/config/ai", {
        enabled: ai.enabled,
        baseUrl: ai.baseUrl,
        model: ai.model,
        ...(aiKey ? { apiKey: aiKey } : {}),
        timeoutSec: Number(ai.timeoutSec) || 60,
      })
      setAiKey("")
      setAiReplacing(false)
      toast("success", t.admin.settings.ai.saved)
    } catch (e) {
      toast("error", (e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  /** AI 配置有效性：基于当前表单 + 已保存 Key 推导（不回显密钥） */
  const aiMissing: string[] = []
  if (ai) {
    if (!ai.baseUrl.trim()) aiMissing.push("base_url")
    if (!ai.model.trim()) aiMissing.push(t.admin.settings.ai.model)
    if (!ai.hasApiKey && !aiKey.trim()) aiMissing.push(t.admin.settings.ai.apiKey)
  }

  return (
    <PageContainer width="narrow">
      <div className="animate-in space-y-5 fade-in duration-300">
        <PageHeader title={t.admin.settings.title} description={t.admin.settings.subtitle} />

        {/* 标签页 */}
        <div className="flex gap-1 rounded-lg border p-1" role="tablist" aria-label={t.admin.settings.title}>
          {tabs.map((item) => (
            <button
              key={item.key}
              role="tab"
              aria-selected={tab === item.key}
              className={cn(
                "flex-1 rounded-md px-3 py-1.5 text-sm transition-colors duration-150",
                tab === item.key ? "bg-brand-muted text-primary font-medium" : "text-muted-foreground hover:text-foreground",
              )}
              onClick={() => setTab(item.key)}
            >
              {item.label}
            </button>
          ))}
        </div>

        {configError ? (
          <ErrorState message={configError} onRetry={() => window.location.reload()} />
        ) : !site || !smtp || !ai ? (
          <LoadingState />
        ) : null}

        {tab === "site" && site ? (
          <FormSection title={t.admin.settings.site.title} description={t.admin.settings.site.desc}>
            <div className="space-y-4">
              <div className="grid gap-4 sm:grid-cols-2">
                <FormField label={t.admin.settings.site.siteName} htmlFor="site-name">
                  <Input id="site-name" value={site.siteName} onChange={(e) => setSite({ ...site, siteName: e.target.value })} />
                </FormField>
                <FormField label={t.admin.settings.site.siteNameEn} htmlFor="site-name-en">
                  <Input id="site-name-en" value={site.siteNameEn} onChange={(e) => setSite({ ...site, siteNameEn: e.target.value })} />
                </FormField>
              </div>
              <FormField label={t.admin.settings.site.announcement} htmlFor="announcement">
                <Input id="announcement" value={site.announcement} onChange={(e) => setSite({ ...site, announcement: e.target.value })} />
              </FormField>
              <div className="grid gap-4 sm:grid-cols-2">
                <FormField label={t.admin.settings.site.pageSize} htmlFor="page-size" help={t.admin.settings.site.pageSizeHelp}>
                  <Input
                    id="page-size"
                    type="number"
                    min={1}
                    max={100}
                    value={site.pageSize}
                    onChange={(e) => setSite({ ...site, pageSize: e.target.value })}
                  />
                </FormField>
                <div className="flex items-center justify-between gap-3 rounded-lg border px-3.5 py-2.5">
                  <div>
                    <Label htmlFor="public-catalog">{t.admin.settings.site.publicCatalog}</Label>
                    <p className="text-muted-foreground mt-0.5 text-xs">{t.admin.settings.site.publicCatalogHelp}</p>
                  </div>
                  <Switch
                    id="public-catalog"
                    checked={site.publicCatalogEnabled}
                    onCheckedChange={(v) => setSite({ ...site, publicCatalogEnabled: v })}
                  />
                </div>
              </div>
              <FormField
                label={t.admin.settings.site.catalogBase}
                htmlFor="catalog-base"
                help={t.admin.settings.site.catalogBaseNote}
              >
                <Input
                  id="catalog-base"
                  value={site.catalogBaseUrl}
                  onChange={(e) => setSite({ ...site, catalogBaseUrl: e.target.value })}
                />
              </FormField>
              <Button disabled={busy} onClick={saveSite}>
                {busy ? t.common.saving : t.common.save}
              </Button>
            </div>
          </FormSection>
        ) : null}

        {tab === "smtp" && smtp ? (
          <div className="space-y-5">
            {/* 连接配置 */}
            <FormSection title={t.admin.settings.smtp.connTitle} description={t.admin.settings.smtp.connDesc}>
              <div className="space-y-4">
                <div className="grid gap-4 sm:grid-cols-[2fr_1fr]">
                  <FormField label={t.admin.settings.smtp.host} htmlFor="smtp-host">
                    <Input id="smtp-host" value={smtp.host} onChange={(e) => setSmtp({ ...smtp, host: e.target.value })} />
                  </FormField>
                  <FormField label={t.admin.settings.smtp.port} htmlFor="smtp-port">
                    <Input id="smtp-port" value={smtp.port} onChange={(e) => setSmtp({ ...smtp, port: e.target.value })} />
                  </FormField>
                </div>
                <div className="grid gap-4 sm:grid-cols-2">
                  <FormField label={t.admin.settings.smtp.username} htmlFor="smtp-username">
                    <Input id="smtp-username" value={smtp.username} onChange={(e) => setSmtp({ ...smtp, username: e.target.value })} />
                  </FormField>
                  <FormField label={t.admin.settings.smtp.encryption} htmlFor="smtp-encryption">
                    <SearchableSelect
                      searchable={false}
                      ariaLabel={t.admin.settings.smtp.encryption}
                      value={smtp.encryption || null}
                      onChange={(v) => setSmtp({ ...smtp, encryption: v })}
                      options={[
                        { value: "starttls", label: t.admin.settings.smtp.encryptionOptions.starttls },
                        { value: "ssl", label: t.admin.settings.smtp.encryptionOptions.ssl },
                        { value: "none", label: t.admin.settings.smtp.encryptionOptions.none },
                      ]}
                    />
                  </FormField>
                </div>
                <div className="grid gap-4 sm:grid-cols-2">
                  <FormField label={t.admin.settings.smtp.fromAddress} htmlFor="smtp-from">
                    <Input id="smtp-from" value={smtp.fromAddress} onChange={(e) => setSmtp({ ...smtp, fromAddress: e.target.value })} />
                  </FormField>
                  <FormField label={t.admin.settings.smtp.fromName} htmlFor="smtp-from-name">
                    <Input id="smtp-from-name" value={smtp.fromName} onChange={(e) => setSmtp({ ...smtp, fromName: e.target.value })} />
                  </FormField>
                </div>
                {/* 密码：不回显，已配置仅显示状态；显式替换流程 */}
                <FormField
                  label={t.admin.settings.smtp.password}
                  htmlFor="smtp-password"
                  help={t.admin.settings.smtp.passwordHelp}
                >
                  {smtpReplacing ? (
                    <div className="flex gap-2">
                      <Input
                        id="smtp-password"
                        type="password"
                        value={smtpPassword}
                        onChange={(e) => setSmtpPassword(e.target.value)}
                        className="flex-1"
                        autoFocus
                      />
                      <Button variant="outline" onClick={() => { setSmtpReplacing(false); setSmtpPassword("") }}>
                        {t.admin.settings.smtp.passwordCancel}
                      </Button>
                    </div>
                  ) : (
                    <div className="flex items-center gap-2.5">
                      <Badge variant={smtp.hasPassword ? "success" : "secondary"}>
                        <KeyRound className="mr-1 size-3" aria-hidden />
                        {smtp.hasPassword ? t.admin.settings.smtp.passwordCurrent : t.admin.settings.smtp.passwordNotSet}
                      </Badge>
                      <Button variant="outline" size="sm" onClick={() => setSmtpReplacing(true)}>
                        {t.admin.settings.smtp.passwordReplace}
                      </Button>
                    </div>
                  )}
                </FormField>
                <Button disabled={busy} onClick={saveSmtp}>
                  {busy ? t.common.saving : t.common.save}
                </Button>
              </div>
            </FormSection>

            {/* 发送测试 */}
            <FormSection title={t.admin.settings.smtp.sendTitle} description={t.admin.settings.smtp.sendDesc}>
              <div className="flex flex-wrap items-end gap-2">
                <FormField label={t.admin.settings.smtp.testTo} htmlFor="smtp-test-to" className="min-w-56 flex-1">
                  <Input
                    id="smtp-test-to"
                    type="email"
                    value={testTo}
                    onChange={(e) => setTestTo(e.target.value)}
                    placeholder="you@example.com"
                  />
                </FormField>
                <Button variant="outline" disabled={busy || !testTo.trim()} onClick={sendTest} className="h-10">
                  {t.admin.settings.smtp.test}
                </Button>
              </div>
            </FormSection>

            {/* 发送记录 */}
            <FormSection title={t.admin.settings.smtp.logsTitle} description={t.admin.settings.smtp.logsDesc} flush>
              {logsError ? <ErrorState className="mx-3 mt-3 py-8" message={logsError} onRetry={loadLogs} /> : null}
              <div className="overflow-x-auto">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>{t.admin.settings.smtp.columns.time}</TableHead>
                      <TableHead>{t.admin.settings.smtp.columns.eventId}</TableHead>
                      <TableHead>{t.admin.settings.smtp.columns.to}</TableHead>
                      <TableHead>{t.admin.settings.smtp.columns.template}</TableHead>
                      <TableHead>{t.admin.settings.smtp.columns.status}</TableHead>
                      <TableHead>{t.admin.settings.smtp.columns.attempts}</TableHead>
                      <TableHead>{t.admin.settings.smtp.columns.error}</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {!logs || logs.items.length === 0 ? (
                      <TableRow>
                        <TableCell colSpan={7} className="text-muted-foreground py-8 text-center text-sm">
                          {t.admin.settings.smtp.logEmpty}
                        </TableCell>
                      </TableRow>
                    ) : (
                      logs.items.map((l, i) => (
                        <TableRow key={i}>
                          <TableCell className="text-muted-foreground text-xs whitespace-nowrap">{fmtTime(l.createdAt)}</TableCell>
                          <TableCell className="font-mono text-xs">{l.eventId}</TableCell>
                          <TableCell className="text-xs">{l.to}</TableCell>
                          <TableCell className="font-mono text-xs">{l.template}</TableCell>
                          <TableCell>
                            {l.status === "failed" ? (
                              <Badge variant="destructive">
                                {l.error?.startsWith("配置错误") ? t.admin.settings.smtp.configErrorTag : t.admin.settings.smtp.logStatus.failed}
                              </Badge>
                            ) : l.status === "sent" ? (
                              <Badge variant="success">{t.admin.settings.smtp.logStatus.sent}</Badge>
                            ) : (
                              <Badge variant="warning">{t.admin.settings.smtp.logStatus.queued}</Badge>
                            )}
                          </TableCell>
                          <TableCell className="text-xs">{l.attempts}</TableCell>
                          <TableCell className="max-w-64 truncate text-xs" title={l.error ?? ""}>
                            {l.error ?? "—"}
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </div>
              {logs ? <Pagination page={logs.page} pageSize={logs.pageSize} total={logs.total} onPage={setLogPage} /> : null}
            </FormSection>
          </div>
        ) : null}

        {tab === "ai" && ai ? (
          <FormSection title={t.admin.settings.ai.title} description={t.admin.settings.ai.desc}>
            <div className="space-y-4">
              <div className="flex items-center justify-between gap-3 rounded-lg border px-3.5 py-2.5">
                <Label htmlFor="ai-enabled">{t.admin.settings.ai.enabled}</Label>
                <Switch id="ai-enabled" checked={ai.enabled} onCheckedChange={(v) => setAi({ ...ai, enabled: v })} />
              </div>
              <FormField label={t.admin.settings.ai.baseUrl} htmlFor="ai-base-url">
                <Input
                  id="ai-base-url"
                  value={ai.baseUrl}
                  placeholder="https://api.openai.com/v1"
                  onChange={(e) => setAi({ ...ai, baseUrl: e.target.value })}
                />
              </FormField>
              <div className="grid gap-4 sm:grid-cols-2">
                <FormField label={t.admin.settings.ai.model} htmlFor="ai-model">
                  <Input id="ai-model" value={ai.model} onChange={(e) => setAi({ ...ai, model: e.target.value })} />
                </FormField>
                <FormField label={t.admin.settings.ai.timeout} htmlFor="ai-timeout">
                  <Input
                    id="ai-timeout"
                    type="number"
                    min={5}
                    max={600}
                    value={ai.timeoutSec}
                    onChange={(e) => setAi({ ...ai, timeoutSec: e.target.value })}
                  />
                </FormField>
              </div>
              {/* API Key：不回显，显式替换 */}
              <FormField label={t.admin.settings.ai.apiKey} htmlFor="ai-key" help={t.admin.settings.ai.apiKeyHelp}>
                {aiReplacing ? (
                  <div className="flex gap-2">
                    <Input
                      id="ai-key"
                      type="password"
                      value={aiKey}
                      onChange={(e) => setAiKey(e.target.value)}
                      className="flex-1"
                      autoFocus
                    />
                    <Button variant="outline" onClick={() => { setAiReplacing(false); setAiKey("") }}>
                      {t.admin.settings.ai.apiKeyCancel}
                    </Button>
                  </div>
                ) : (
                  <div className="flex items-center gap-2.5">
                    <Badge variant={ai.hasApiKey ? "success" : "secondary"}>
                      <KeyRound className="mr-1 size-3" aria-hidden />
                      {ai.hasApiKey ? t.admin.settings.ai.apiKeySet : t.admin.settings.ai.apiKeyNotSet}
                    </Badge>
                    <Button variant="outline" size="sm" onClick={() => setAiReplacing(true)}>
                      {t.admin.settings.ai.apiKeyReplace}
                    </Button>
                  </div>
                )}
              </FormField>
              {/* 配置有效性 */}
              <div className="rounded-lg border p-3">
                <div className="mb-1 text-xs font-medium">{t.admin.settings.ai.validityTitle}</div>
                {aiMissing.length > 0 ? (
                  <p className="text-destructive flex items-start gap-1.5 text-xs leading-relaxed">
                    <XCircle className="mt-0.5 size-3.5 shrink-0" aria-hidden />
                    {t.admin.settings.ai.invalid(aiMissing.join(" / "))}
                  </p>
                ) : !ai.enabled ? (
                  <p className="text-muted-foreground flex items-start gap-1.5 text-xs">
                    <CheckCircle2 className="text-success mt-0.5 size-3.5 shrink-0" aria-hidden />
                    {t.admin.settings.ai.validWhenEnabled}
                  </p>
                ) : (
                  <p className="text-success flex items-start gap-1.5 text-xs">
                    <CheckCircle2 className="mt-0.5 size-3.5 shrink-0" aria-hidden />
                    {t.admin.settings.ai.valid}
                  </p>
                )}
              </div>
              <p className="text-muted-foreground text-xs leading-relaxed">{t.admin.settings.ai.note}</p>
              <Button disabled={busy} onClick={saveAi}>
                {busy ? t.common.saving : t.common.save}
              </Button>
            </div>
          </FormSection>
        ) : null}

        {/* 权限标签（宿主权限表） */}
        {tab === "perms" ? (
          <FormSection
            title={t.admin.settings.perms.title}
            description={t.admin.settings.perms.desc}
            flush
          >
            <div className="flex items-center justify-between px-5 pt-3">
              <span className="text-muted-foreground inline-flex items-center gap-1.5 text-xs">
                <Tags className="size-3.5" aria-hidden />
                {perms ? t.common.total(perms.length) : ""}
              </span>
              <Button
                size="sm"
                onClick={() => setPermDialog({ mode: "add", name: "", risk: "low", description: "" })}
              >
                <Plus className="size-3.5" aria-hidden />
                {t.admin.settings.perms.add}
              </Button>
            </div>
            <div className="overflow-x-auto px-3 pb-3">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>{t.admin.settings.perms.name}</TableHead>
                    <TableHead>{t.admin.settings.perms.risk}</TableHead>
                    <TableHead>{t.admin.settings.perms.description}</TableHead>
                    <TableHead className="text-right">{t.admin.settings.perms.usedBy}</TableHead>
                    <TableHead className="w-24 text-right">{t.me.columns.actions}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {!perms ? (
                    <TableRow>
                      <TableCell colSpan={5} className="py-8">
                        <LoadingState className="py-6" />
                      </TableCell>
                    </TableRow>
                  ) : perms.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={5} className="text-muted-foreground py-8 text-center text-sm">
                        {t.admin.settings.perms.empty}
                      </TableCell>
                    </TableRow>
                  ) : (
                    perms.map((p) => (
                      <TableRow key={p.name}>
                        <TableCell className="font-mono text-xs">{p.name}</TableCell>
                        <TableCell>
                          <Badge
                            variant={p.risk === "high" ? "destructive" : p.risk === "medium" ? "warning" : "secondary"}
                          >
                            {t.permission.risk[p.risk]}
                          </Badge>
                        </TableCell>
                        <TableCell className="max-w-72 text-xs whitespace-normal">{p.description || "—"}</TableCell>
                        <TableCell className="text-right font-mono text-xs">
                          {p.usedBy > 0 ? <Badge variant="outline">{p.usedBy}</Badge> : "—"}
                        </TableCell>
                        <TableCell className="text-right">
                          <div className="flex items-center justify-end gap-1">
                            <Button
                              variant="outline"
                              size="icon-xs"
                              aria-label={t.admin.settings.perms.edit}
                              title={t.admin.settings.perms.edit}
                              onClick={() =>
                                setPermDialog({ mode: "edit", name: p.name, risk: p.risk, description: p.description })
                              }
                            >
                              <Pencil className="size-3.5" />
                            </Button>
                            <Button
                              variant="outline"
                              size="icon-xs"
                              className="text-destructive hover:bg-destructive/10 hover:text-destructive"
                              aria-label={t.admin.settings.perms.delete}
                              title={p.usedBy > 0 ? `${t.admin.settings.perms.usedBy}: ${p.usedBy}` : t.admin.settings.perms.delete}
                              disabled={p.usedBy > 0}
                              onClick={() => setDeleteTarget(p)}
                            >
                              <XCircle className="size-3.5" />
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
            </div>
          </FormSection>
        ) : null}
      </div>

      {/* 权限标签 新增/编辑 */}
      <Dialog
        open={!!permDialog}
        onClose={() => setPermDialog(null)}
        title={permDialog?.mode === "add" ? t.admin.settings.perms.add : t.admin.settings.perms.edit}
        footer={
          <>
            <Button variant="outline" onClick={() => setPermDialog(null)}>
              {t.common.cancel}
            </Button>
            <Button
              disabled={
                permBusy ||
                !permDialog ||
                !PERMISSION_NAME_RE.test(permDialog.name.trim()) ||
                (permDialog.mode === "add" && perms?.some((p) => p.name === permDialog.name.trim()))
              }
              onClick={savePerm}
            >
              {permBusy ? t.common.saving : t.common.confirm}
            </Button>
          </>
        }
      >
        {permDialog ? (
          <div className="space-y-3">
            <FormField
              label={t.admin.settings.perms.name}
              htmlFor="perm-name"
              help={permDialog.mode === "add" ? t.admin.settings.perms.nameHelp : undefined}
              error={
                permDialog.name && !PERMISSION_NAME_RE.test(permDialog.name.trim())
                  ? t.admin.settings.perms.nameHelp
                  : permDialog.mode === "add" && perms?.some((p) => p.name === permDialog.name.trim())
                    ? t.admin.settings.perms.nameTaken
                    : undefined
              }
            >
              <Input
                id="perm-name"
                value={permDialog.name}
                disabled={permDialog.mode === "edit"}
                placeholder="screen_capture"
                onChange={(e) => setPermDialog({ ...permDialog, name: e.target.value })}
                className="font-mono"
              />
            </FormField>
            <FormField label={t.admin.settings.perms.risk}>
              <SearchableSelect
                searchable={false}
                ariaLabel={t.admin.settings.perms.risk}
                value={permDialog.risk}
                onChange={(v) => setPermDialog({ ...permDialog, risk: v as "low" | "medium" | "high" })}
                options={[
                  { value: "high", label: t.permission.risk.high },
                  { value: "medium", label: t.permission.risk.medium },
                  { value: "low", label: t.permission.risk.low },
                ]}
              />
            </FormField>
            <FormField label={t.admin.settings.perms.description} htmlFor="perm-desc" help={t.admin.settings.perms.descriptionHelp}>
              <Textarea
                id="perm-desc"
                rows={3}
                value={permDialog.description}
                onChange={(e) => setPermDialog({ ...permDialog, description: e.target.value })}
              />
            </FormField>
          </div>
        ) : null}
      </Dialog>

      {/* 删除确认 */}
      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onConfirm={doDeletePerm}
        title={t.admin.settings.perms.deleteTitle}
        confirmText={t.admin.settings.perms.delete}
      >
        {deleteTarget ? <span className="font-mono text-xs">{deleteTarget.name}</span> : null}
        {deleteTarget ? <p className="text-muted-foreground mt-1.5 text-sm leading-relaxed">{t.admin.settings.perms.deleteConfirm(deleteTarget.name)}</p> : null}
      </ConfirmDialog>
    </PageContainer>
  )
}
