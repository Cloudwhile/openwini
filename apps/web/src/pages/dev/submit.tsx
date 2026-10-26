import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useNavigate, useParams } from "react-router-dom"
import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  CheckCircle2,
  ExternalLink,
  GitBranch,
  Loader2,
  RefreshCw,
  XCircle,
} from "lucide-react"
import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@workspace/ui/components/card"
import { Input } from "@workspace/ui/components/input"
import { Textarea } from "@workspace/ui/components/textarea"
import {
  ApiError,
  api,
  apiErrMessage,
  type DevPluginDetail,
  type ManifestPreview,
  type PermissionInfo,
  type RepoItem,
  type RepoPage,
  type RepoValidation,
} from "@/lib/api"
import { useToast } from "@/components/toast"
import { PermissionBadges } from "@/components/permission-badge"
import { SearchableSelect, type SearchOption } from "@/components/searchable-select"
import { ErrorState, LoadingState } from "@/components/page-states"
import { PageContainer, PageHeader, FormField } from "@/components/page-header"
import { cn } from "@workspace/ui/lib/utils"
import { t } from "@/i18n"

type Mode = "new" | "edit" | "newVersion" | "official" | "officialVersion"
/** official / officialVersion：直发向导（终点为「发布」，不进审核队列） */
const isOfficial = (m: Mode) => m === "official" || m === "officialVersion"

/** 步骤条：带描述的三步向导 */
function Stepper({ step, labels, descs }: { step: 1 | 2 | 3; labels: string[]; descs: string[] }) {
  return (
    <ol className="flex items-start gap-1 sm:gap-2" aria-label={t.submit.title}>
      {labels.map((label, i) => {
        const n = (i + 1) as 1 | 2 | 3
        const active = step === n
        const done = step > n
        return (
          <li key={n} className="flex min-w-0 flex-1 items-start gap-2" aria-current={active ? "step" : undefined}>
            <span
              className={cn(
                "mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-full text-xs font-medium transition-colors duration-200",
                done
                  ? "bg-success/15 text-success"
                  : active
                    ? "bg-primary text-primary-foreground"
                    : "bg-muted text-muted-foreground",
              )}
            >
              {done ? <CheckCircle2 className="size-3.5" aria-hidden /> : n}
            </span>
            <span className="min-w-0">
              <span className={cn("block truncate text-xs", active ? "text-foreground font-medium" : "text-muted-foreground")}>
                {label}
              </span>
              <span className="text-muted-foreground/80 hidden truncate text-[11px] sm:block">{descs[i]}</span>
            </span>
            {i < labels.length - 1 ? <div className="bg-border mt-3.5 h-px min-w-2 flex-1" aria-hidden /> : null}
          </li>
        )
      })}
    </ol>
  )
}

/** 校验项：通过/失败双态行 */
function CheckRow({ ok, text }: { ok: boolean; text: string }) {
  return (
    <div className={cn("flex items-start gap-2 text-sm", ok ? "text-muted-foreground" : "text-destructive")}>
      {ok ? (
        <CheckCircle2 className="text-success mt-0.5 size-4 shrink-0" aria-hidden />
      ) : (
        <XCircle className="mt-0.5 size-4 shrink-0" aria-hidden />
      )}
      <span>{text}</span>
    </div>
  )
}

export function SubmitPage({ mode }: { mode: Mode }) {
  const { pluginId: routePluginId } = useParams<{ pluginId: string }>()
  const navigate = useNavigate()
  const { toast } = useToast()

  const [pluginId, setPluginId] = useState(mode === "new" ? "" : (routePluginId ?? ""))
  const [repoUrl, setRepoUrl] = useState("")
  const [repoItems, setRepoItems] = useState<RepoItem[]>([])
  const [reposLoading, setReposLoading] = useState(false)
  const [validation, setValidation] = useState<RepoValidation | null>(null)
  const [preview, setPreview] = useState<ManifestPreview | null>(null)
  // manifest 读取失败的可操作错误（未提交到仓库 / 无法获取）
  const [previewError, setPreviewError] = useState("")
  const [version, setVersion] = useState("")
  const [homepage, setHomepage] = useState("")
  const [docs, setDocs] = useState("")
  // 英文描述：清单带 description_en 时锁定为清单值，否则由开发者填写（必填）
  const [descriptionEn, setDescriptionEn] = useState("")
  const [known, setKnown] = useState<Record<string, PermissionInfo> | null>(null)
  const [diffs, setDiffs] = useState<string[]>([])
  const [busy, setBusy] = useState(false)
  const repoImmutable = mode === "newVersion" || mode === "officialVersion"
  const [loaded, setLoaded] = useState(mode === "new" || mode === "official")
  const [step, setStep] = useState<1 | 2 | 3>(1)
  const [loadError, setLoadError] = useState("")
  const [loadAttempt, setLoadAttempt] = useState(0)
  const [draftCreated, setDraftCreated] = useState(false)
  const repoRequest = useRef(0)
  const goStep = (n: 1 | 2 | 3) => {
    setStep(n)
    window.scrollTo({ top: 0 })
  }

  useEffect(() => {
    api
      .get<{ permissions: PermissionInfo[] }>("/api/public/permissions")
      .then((r) => {
        const m: Record<string, PermissionInfo> = {}
        for (const p of r.permissions) m[p.name] = p
        setKnown(m)
      })
      .catch(() => {})
  }, [])

  // 编辑 / 新版本 / 直发模式：载入既有插件信息
  useEffect(() => {
    if (mode === "new" || mode === "official" || !routePluginId) return
    let active = true
    api
      .get<DevPluginDetail>(`/api/dev/plugins/${encodeURIComponent(routePluginId)}`)
      .then((d) => {
        if (!active) return
        setRepoUrl(d.repositoryUrl)
        setHomepage(d.homepageUrl ?? "")
        setDocs(d.docsUrl ?? "")
        setLoaded(true)
        setLoadError("")
      })
      .catch((e) => { if (active) setLoadError(apiErrMessage(e)) })
    return () => { active = false }
  }, [mode, routePluginId, loadAttempt])

  // 一次性自动拉取全部分页（上限 5 页 × 100 = 500 个仓库）供选择器搜索，
  // 不再向用户暴露手动翻页控件
  const loadRepos = useCallback(async () => {
    setReposLoading(true)
    try {
      const all: RepoItem[] = []
      for (let page = 1; page <= 5; page++) {
        const r: RepoPage = await api.get<RepoPage>(`/api/dev/repos?page=${page}`)
        all.push(...r.items)
        if (!r.hasNext) break
      }
      setRepoItems(all)
    } catch (e) {
      toast("error", apiErrMessage(e))
    } finally {
      setReposLoading(false)
    }
  }, [toast])

  useEffect(() => {
    if (repoImmutable || repoItems.length > 0) return
    // 延迟到下一轮宏任务触发，避免在 effect 中同步 setState（reposLoading）
    const id = setTimeout(() => void loadRepos(), 0)
    return () => clearTimeout(id)
  }, [repoImmutable, repoItems.length, loadRepos])

  const selectRepo = async (url: string) => {
    const requestId = ++repoRequest.current
    setRepoUrl(url)
    setValidation(null)
    setPreview(null)
    setPreviewError("")
    setVersion("")
    setDiffs([])
    setDescriptionEn("")
    try {
      const v = await api.post<RepoValidation>("/api/dev/repos/validate", { repositoryUrl: url })
      if (requestId !== repoRequest.current) return
      setValidation(v)
      if (v.ok) {
        try {
          const p = await api.post<ManifestPreview>("/api/dev/manifest-preview", { repositoryUrl: url })
          if (requestId === repoRequest.current) setPreview(p)
        } catch (e) {
          // manifest 读取失败（未提交/无法获取）：显式记录（按错误码本地化），禁止进入下一步
          if (requestId === repoRequest.current) setPreviewError(apiErrMessage(e))
        }
      }
    } catch (e) {
      if (requestId === repoRequest.current) setPreviewError(apiErrMessage(e))
    }
  }

  // 清单 description_en 不再同步进 state：提交时经 effectiveDescriptionEn 统一生效
  // 编辑 / 新版本模式：进入页面时自动对既有仓库执行校验与清单预览
  // 依赖只含 [mode, repoUrl]；重置放在请求回调内，避免在 effect 中同步 setState。
  useEffect(() => {
    if (mode === "new" || mode === "official" || !repoUrl) return
    let cancelled = false
    api
      .post<RepoValidation>("/api/dev/repos/validate", { repositoryUrl: repoUrl })
      .then((v) => {
        if (cancelled) return
        setValidation(v)
        if (v.ok) {
          api
            .post<ManifestPreview>("/api/dev/manifest-preview", { repositoryUrl: repoUrl })
            .then((p) => {
              if (!cancelled) setPreview(p)
            })
            .catch((e: Error) => {
              // manifest 读取失败（未提交/无法获取）：显式记录（按错误码本地化），禁止进入下一步
              if (!cancelled) setPreviewError(apiErrMessage(e))
            })
        }
      })
      .catch((e: Error) => {
        // 校验请求本身失败（网络/令牌问题）：给出错误而非静默转圈
        if (!cancelled) setPreviewError(apiErrMessage(e))
      })
    return () => {
      cancelled = true
    }
  }, [mode, repoUrl])

  const manifest = preview?.manifest
  // 英文描述生效值：清单带 description_en 时以清单为准（锁定），否则取用户输入
  const effectiveDescriptionEn = manifest?.description_en || descriptionEn
  const releases = useMemo(
    () => (preview?.releases ?? []).filter((r) => r.zipAssets.length > 0),
    [preview],
  )

  const saveDraft = async () => {
    setBusy(true)
    try {
      if (mode === "new" && !draftCreated) {
        await api.post("/api/dev/plugins", {
          pluginId: pluginId.trim(),
          repositoryUrl: repoUrl,
          homepageUrl: homepage || null,
          docsUrl: docs || null,
        })
        setDraftCreated(true)
        toast("success", t.misc.draftSaved)
        navigate(`/me/plugins/${encodeURIComponent(pluginId.trim())}`)
      } else if (routePluginId || draftCreated) {
        const id = routePluginId ?? pluginId.trim()
        await api.put(`/api/dev/plugins/${encodeURIComponent(id)}`, {
          repositoryUrl: repoUrl,
          homepageUrl: homepage || null,
          docsUrl: docs || null,
        })
        toast("success", t.misc.saved)
        navigate(`/me/plugins/${encodeURIComponent(id)}`)
      }
    } catch (e) {
      toast("error", (e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const submit = async () => {
    if (busy) return // 防重复提交
    if (!manifest?.description) {
      toast("error", t.misc.manifestMissing)
      return
    }
    setBusy(true)
    setDiffs([])
    try {
      // 直发：走管理接口，直接发布（不进审核队列）；描述以清单为准
      if (isOfficial(mode)) {
        const pid = mode === "official" ? pluginId.trim() : (routePluginId ?? "")
        const path =
          mode === "official"
            ? "/api/admin/official/plugins"
            : `/api/admin/official/plugins/${encodeURIComponent(pid)}/versions`
        await api.post(path, {
          pluginId: pid,
          repositoryUrl: repoUrl,
          version,
          description: manifest.description ?? "",
          homepageUrl: homepage || null,
          docsUrl: docs || null,
        })
        toast("success", t.misc.officialPublished)
        navigate("/admin/plugins")
        return
      }
      const body = {
        version,
        description: manifest.description ?? "",
        descriptionEn: effectiveDescriptionEn,
        repositoryUrl: repoUrl,
        homepageUrl: homepage || null,
        docsUrl: docs || null,
      }
      if (mode === "new") {
        // 创建成功后保留状态，后续提交失败可在本页直接修正并重试。
        if (!draftCreated) {
          await api.post("/api/dev/plugins", {
            pluginId: pluginId.trim(),
            repositoryUrl: repoUrl,
            homepageUrl: homepage || null,
            docsUrl: docs || null,
          })
          setDraftCreated(true)
        }
        await api.post(`/api/dev/plugins/${encodeURIComponent(pluginId.trim())}/submit`, body)
      } else if (mode === "newVersion" && routePluginId) {
        await api.post(`/api/dev/plugins/${encodeURIComponent(routePluginId)}/versions`, body)
      } else if (routePluginId) {
        await api.post(`/api/dev/plugins/${encodeURIComponent(routePluginId)}/submit`, body)
      }
      toast("success", t.misc.submitted)
      navigate(`/me/plugins/${encodeURIComponent(mode === "new" ? pluginId.trim() : (routePluginId ?? ""))}`)
    } catch (e) {
      if (e instanceof ApiError) {
        const ds = e.differences
        if (ds) setDiffs(ds)
        else toast("error", apiErrMessage(e))
      } else {
        toast("error", String(e))
      }
    } finally {
      setBusy(false)
    }
  }

  const title =
    mode === "new"
      ? t.submit.title
      : mode === "edit"
        ? t.submit.editTitle
        : mode === "official"
          ? t.submit.officialTitle
          : mode === "officialVersion"
            ? t.submit.officialVersionTitle
            : t.submit.newVersionTitle

  const canSubmit =
    !!version &&
    !!manifest &&
    !!validation?.ok &&
    (!repoImmutable || repoUrl !== "") &&
    (mode === "new" || mode === "official" ? pluginId.trim().length > 0 : true) &&
    // 英文描述必填：清单未带 description_en 时必须由开发者填写；直发以清单为准
    (isOfficial(mode) || !!(manifest.description_en || descriptionEn.trim()))

  if (loadError) return (
    <PageContainer width="narrow">
      <ErrorState message={loadError} onRetry={() => setLoadAttempt((value) => value + 1)} />
    </PageContainer>
  )

  if (!loaded)
    return (
      <PageContainer width="narrow" className="py-10">
        <LoadingState />
      </PageContainer>
    )

  const stepLabels = [t.submit.step1, t.submit.step2, t.submit.step3]
  const stepDescs = [t.submit.step1Desc, t.submit.step2Desc, t.submit.step3Desc]
  const canNext1 = validation?.ok === true && !!preview && !previewError
  const idMismatch = (mode === "new" || mode === "official") && !!manifest?.id && !!pluginId && pluginId !== manifest.id
  const versionMismatch = !!manifest?.version && !!version && version !== manifest.version
  const canNext2 =
    !!preview &&
    !!version &&
    !idMismatch &&
    !versionMismatch &&
    (mode !== "new" || pluginId.trim().length > 0) &&
    (isOfficial(mode) || !!(manifest?.description_en || descriptionEn.trim()))

  // 仓库选择器选项：所有者前缀 + 公开/权限徽标 + 不可选原因
  const repoOptions: SearchOption[] = repoItems.map((r) => {
    const usable = r.isAdmin && !r.private
    return {
      value: r.url,
      label: r.fullName,
      description: r.description ?? undefined,
      keywords: r.fullName,
      disabled: !usable,
      disabledReason: !r.isAdmin
        ? t.submit.repoChecks.noAdmin
        : r.private
          ? t.submit.repoChecks.private
          : undefined,
      badge: (
        <span className="flex shrink-0 items-center gap-1">
          {r.private ? <Badge variant="outline">{t.misc.repoPrivateSuffix.trim()}</Badge> : null}
          {r.isAdmin ? <Badge variant="success">{t.submit.repoChecks.admin}</Badge> : null}
        </span>
      ),
    }
  })

  const selectedRepo = repoItems.find((r) => r.url === repoUrl)

  return (
    <PageContainer width="narrow">
      <div className="animate-in space-y-6 fade-in duration-300">
        <PageHeader title={title} description={t.submit.singleRepoNote} />

        <Stepper step={step} labels={stepLabels} descs={stepDescs} />

        {/* 第 1 步：仓库选择（可搜索选择器） */}
        {step === 1 ? (
          <Card>
            <CardHeader>
              <CardTitle>{t.submit.step1}</CardTitle>
              <CardDescription>{t.submit.step1Desc}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              {repoImmutable ? (
                <div className="flex items-center gap-2 text-sm">
                  <Input value={repoUrl} disabled aria-label={t.submit.repoLabel} />
                  <Badge variant="secondary">{t.submit.repoImmutable}</Badge>
                </div>
              ) : (
                <>
                  <div className="flex items-end gap-2">
                    <FormField label={t.submit.repoLabel} className="flex-1">
                      <SearchableSelect
                        options={repoOptions}
                        value={repoUrl || null}
                        onChange={(v) => void selectRepo(v)}
                        placeholder={t.submit.repoPlaceholder}
                        searchPlaceholder={t.submit.repoSearchPlaceholder}
                        emptyText={t.submit.repoEmpty}
                        ariaLabel={t.submit.repoLabel}
                      />
                    </FormField>
                    <Button
                      variant="outline"
                      size="icon"
                      className="size-10 shrink-0"
                      aria-label={t.submit.repoRefresh}
                      title={t.submit.repoRefresh}
                      disabled={reposLoading}
                      onClick={() => void loadRepos()}
                    >
                      <RefreshCw className={cn("size-4", reposLoading && "animate-spin")} />
                    </Button>
                  </div>
                </>
              )}
              {/* 校验与 manifest 读取状态：可选仓库与锁定仓库（新版本）两分支共享 */}
              {repoUrl && !validation ? (
                <div className="text-muted-foreground flex items-center gap-2 text-sm">
                  <Loader2 className="size-4 animate-spin" aria-hidden /> {t.misc.validating}
                </div>
              ) : null}
              {validation?.ok && !preview && !previewError ? (
                <div className="text-muted-foreground flex items-center gap-2 text-sm">
                  <Loader2 className="size-4 animate-spin" aria-hidden />
                  {t.misc.readingManifest}
                </div>
              ) : null}
              {previewError ? (
                <div role="alert" className="border-destructive/40 bg-destructive/5 space-y-1.5 rounded-lg border p-3 text-sm">
                  <div className="text-destructive flex items-start gap-2 font-medium">
                    <XCircle className="mt-0.5 size-4 shrink-0" aria-hidden />
                    {t.misc.manifestReadFailed}
                  </div>
                  <p className="text-destructive/90 leading-relaxed">{previewError}</p>
                  <p className="text-muted-foreground text-xs">{t.misc.manifestFixHint}</p>
                </div>
              ) : null}
              {validation ? (
                <div className="space-y-1.5 rounded-lg border p-3">
                  <div className="flex items-center gap-2 font-medium">
                    {validation.ok ? (
                      <>
                        <CheckCircle2 className="text-success size-4" aria-hidden />
                        {t.submit.repoChecks.ok}
                      </>
                    ) : (
                      <>
                        <XCircle className="text-destructive size-4" aria-hidden />
                        {t.misc.validationFailed}
                      </>
                    )}
                  </div>
                  <CheckRow ok={!validation.private} text={validation.private ? t.submit.repoChecks.private : t.submit.repoChecks.public} />
                  <CheckRow ok={validation.hasAdmin} text={validation.hasAdmin ? t.submit.repoChecks.admin : t.submit.repoChecks.noAdmin} />
                  <CheckRow
                    ok={validation.hasReleaseZip}
                    text={validation.hasReleaseZip ? t.submit.repoChecks.zip : t.submit.repoChecks.noZip}
                  />
                </div>
              ) : null}
            </CardContent>
            <div className="flex items-center justify-end gap-2 border-t px-6 py-4">
              <Button disabled={!canNext1} onClick={() => goStep(2)}>
                {t.submit.nextStep}
                <ArrowRight className="size-4" aria-hidden />
              </Button>
            </div>
          </Card>
        ) : null}

        {/* 第 2 步：验证插件 */}
        {step === 2 && !preview && previewError ? (
          <Card>
            <CardContent className="space-y-3 py-10 text-sm">
              <div className="text-destructive flex items-start gap-2 font-medium">
                <XCircle className="mt-0.5 size-4 shrink-0" aria-hidden />
                {t.misc.manifestReadFailed}
              </div>
              <p className="text-destructive/90 leading-relaxed">{previewError}</p>
              <Button variant="outline" size="sm" onClick={() => goStep(1)}>
                <ArrowLeft className="size-4" aria-hidden />
                {t.submit.prevStep}
              </Button>
            </CardContent>
          </Card>
        ) : null}
        {step === 2 && !preview && !previewError ? (
          <Card>
            <CardContent className="text-muted-foreground flex items-center justify-center gap-2 py-16 text-sm">
              <Loader2 className="size-4 animate-spin" aria-hidden />
              {t.common.loading}
            </CardContent>
          </Card>
        ) : null}
        {step === 2 && preview && manifest ? (
          <Card>
            <CardHeader>
              <CardTitle>{t.submit.step2}</CardTitle>
              <CardDescription>
                {t.submit.manifestOk}
                {t.misc.branchNote(preview.defaultBranch)}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-5">
              {/* 概要信息 */}
              <div className="grid gap-3 sm:grid-cols-2">
                <FormField label={t.submit.verify.id}>
                  {mode === "new" || mode === "official" ? (
                    <Input
                      value={pluginId}
                      placeholder={t.submit.pluginIdPlaceholder}
                      onChange={(e) => setPluginId(e.target.value)}
                      aria-invalid={idMismatch || undefined}
                    />
                  ) : (
                    <Input value={pluginId} disabled />
                  )}
                </FormField>
                <FormField label={t.submit.version}>
                  <SearchableSelect
                    options={releases.map((r) => ({
                      value: r.tag.replace(/^v/, ""),
                      label: r.tag,
                      description: r.zipAssets.map((a) => a.name).join("、"),
                      keywords: r.tag,
                    }))}
                    value={version || null}
                    onChange={setVersion}
                    placeholder={t.misc.chooseRelease}
                    emptyText={t.submit.verify.releaseNone}
                    ariaLabel={t.submit.releasePicker}
                  />
                </FormField>
              </div>

              {/* 校验结果面板：必须修复 vs 通过项 */}
              <div className="space-y-2">
                <h3 className="text-sm font-semibold">{t.submit.verify.title}</h3>
                <div className="rounded-lg border p-3">
                  <div className="grid gap-1.5">
                    <CheckRow ok={!idMismatch} text={idMismatch ? t.misc.mismatchId(manifest.id ?? "") : t.submit.verify.passed} />
                    <CheckRow ok={!versionMismatch} text={versionMismatch ? t.misc.mismatchVersion(manifest.version ?? "") : t.submit.verify.passed} />
                    <CheckRow
                      ok={isOfficial(mode) || !!manifest.description_en || !!descriptionEn.trim()}
                      text={
                        isOfficial(mode)
                          ? t.admin.plugins.officialDescEnHint
                          : manifest.description_en || descriptionEn.trim()
                            ? t.submit.verify.passed
                            : t.submit.descriptionEn
                      }
                    />
                    <CheckRow ok={releases.length > 0} text={releases.length > 0 ? t.submit.repoChecks.zip : t.submit.verify.releaseNone} />
                  </div>
                  {(!canNext2 && step === 2) || (idMismatch || versionMismatch) ? (
                    <p className="text-destructive mt-2 flex items-start gap-1.5 border-t pt-2 text-xs leading-relaxed">
                      <AlertTriangle className="mt-0.5 size-3.5 shrink-0" aria-hidden />
                      {t.submit.verify.mustFix}
                    </p>
                  ) : null}
                </div>
              </div>

              {/* 清单信息 */}
              <div className="grid gap-3 sm:grid-cols-2">
                <FormField label={t.submit.manifestName}>
                  <Input value={manifest.name ?? ""} disabled />
                </FormField>
                <FormField label={t.submit.manifestMinHost}>
                  <Input value={manifest.minHostVersion ?? ""} disabled />
                </FormField>
                <FormField label={t.submit.manifestContract}>
                  <Input value={String(manifest.contractVersion ?? "")} disabled />
                </FormField>
                <FormField label={t.submit.verify.repo}>
                  <a
                    href={repoUrl}
                    target="_blank"
                    rel="noreferrer noopener"
                    className="hover:bg-muted flex h-10 items-center gap-1.5 rounded-md border px-3 text-sm hover:underline"
                  >
                    <GitBranch className="text-muted-foreground size-3.5 shrink-0" aria-hidden />
                    <span className="truncate">{selectedRepo?.fullName ?? repoUrl.replace("https://github.com/", "")}</span>
                    <ExternalLink className="text-muted-foreground ml-auto size-3 shrink-0" aria-hidden />
                  </a>
                </FormField>
              </div>
              <FormField label={t.submit.manifestPermissions}>
                <PermissionBadges permissions={manifest.permissions ?? []} known={known} />
              </FormField>
              <FormField label={t.submit.manifestDescription}>
                <Textarea value={manifest.description ?? ""} disabled rows={5} />
              </FormField>
              <FormField
                label={manifest.description_en ? t.submit.descriptionEnFromManifest : t.submit.descriptionEn}
                help={!manifest.description_en && !isOfficial(mode) ? t.submit.verify.mustFix : undefined}
              >
                {manifest.description_en ? (
                  <Textarea value={manifest.description_en} disabled rows={4} />
                ) : isOfficial(mode) ? (
                  /* 直发以清单为准：清单未提供 description_en 时仅提示，不强制填写 */
                  <p className="text-muted-foreground rounded-lg border border-dashed p-3 text-xs">
                    {t.admin.plugins.officialDescEnHint}
                  </p>
                ) : (
                  <Textarea
                    value={descriptionEn}
                    rows={4}
                    placeholder="English description of the plugin…"
                    onChange={(e) => setDescriptionEn(e.target.value)}
                  />
                )}
              </FormField>
            </CardContent>
            <div className="flex items-center justify-between gap-2 border-t px-6 py-4">
              <Button variant="outline" onClick={() => goStep(1)}>
                <ArrowLeft className="size-4" aria-hidden />
                {t.submit.prevStep}
              </Button>
              <Button disabled={!canNext2} onClick={() => goStep(3)}>
                {t.submit.nextStep}
                <ArrowRight className="size-4" aria-hidden />
              </Button>
            </div>
          </Card>
        ) : null}

        {/* 第 3 步：确认提交（集中展示全部内容） */}
        {step === 3 ? (
          <Card>
            <CardHeader>
              <CardTitle>{t.submit.step3}</CardTitle>
              <CardDescription>{t.submit.confirm.note}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-5">
              {/* 提交内容总览 */}
              <dl className="divide-y rounded-lg border">
                {[
                  {
                    label: t.submit.verify.id,
                    value: <span className="font-mono text-xs">{mode === "new" || mode === "official" ? pluginId : pluginId}</span>,
                  },
                  {
                    label: t.submit.verify.nameVersion,
                    value: (
                      <span className="text-xs">
                        {manifest?.name ?? "—"} · <span className="font-mono">v{version}</span>
                      </span>
                    ),
                  },
                  {
                    label: t.submit.verify.repo,
                    value: (
                      <a className="flex items-center gap-1 truncate text-xs hover:underline" href={repoUrl} target="_blank" rel="noreferrer noopener">
                        {selectedRepo?.fullName ?? repoUrl.replace("https://github.com/", "")}
                        <ExternalLink className="size-3 shrink-0" aria-hidden />
                      </a>
                    ),
                  },
                  {
                    label: t.submit.verify.perms,
                    value: <PermissionBadges permissions={manifest?.permissions ?? []} known={known} />,
                  },
                  {
                    label: t.submit.confirm.dist,
                    value: (
                      <span className="text-xs">
                        {releases.find((r) => r.tag.replace(/^v/, "") === version)?.zipAssets.map((a) => a.name).join("、") ?? "—"}
                      </span>
                    ),
                  },
                ].map((row) => (
                  <div key={row.label} className="flex items-start justify-between gap-4 px-3.5 py-2.5">
                    <dt className="text-muted-foreground shrink-0 text-sm">{row.label}</dt>
                    <dd className="min-w-0 text-right">{row.value}</dd>
                  </div>
                ))}
              </dl>

              <FormField label={t.submit.homepage}>
                <Input value={homepage} placeholder="https://" onChange={(e) => setHomepage(e.target.value)} aria-label={t.submit.homepage} />
              </FormField>
              <FormField label={t.submit.docs}>
                <Input value={docs} placeholder="https://" onChange={(e) => setDocs(e.target.value)} aria-label={t.submit.docs} />
              </FormField>

              {diffs.length > 0 ? (
                <div role="alert" className="border-destructive/40 bg-destructive/5 rounded-lg border p-3 text-sm">
                  <div className="mb-1 font-medium">{t.submit.differences}</div>
                  <ul className="list-disc space-y-1 pl-5">
                    {diffs.map((d, i) => (
                      <li key={i}>{d}</li>
                    ))}
                  </ul>
                </div>
              ) : null}

              <div className="flex flex-wrap items-center gap-2 border-t pt-4">
                <Button variant="outline" onClick={() => goStep(2)}>
                  <ArrowLeft className="size-4" aria-hidden />
                  {t.submit.prevStep}
                </Button>
                {mode !== "newVersion" && !isOfficial(mode) ? (
                  <Button variant="outline" disabled={busy || !pluginId.trim() || !repoUrl} onClick={saveDraft}>
                    {t.submit.saveDraft}
                  </Button>
                ) : null}
                <Button disabled={busy || !canSubmit} onClick={submit}>
                  {busy ? (
                    <>
                      <Loader2 className="size-4 animate-spin" aria-hidden />
                      {t.submit.confirm.submitting}
                    </>
                  ) : null}
                  {!busy
                    ? isOfficial(mode)
                      ? t.misc.publish
                      : mode === "new"
                        ? t.submit.submitReview
                        : mode === "edit"
                          ? t.submit.resubmit
                          : t.submit.publishNewVersion
                    : null}
                </Button>
                {busy ? <span className="text-muted-foreground text-xs">{t.submit.confirm.duplicateGuard}</span> : null}
              </div>
            </CardContent>
          </Card>
        ) : null}
      </div>
    </PageContainer>
  )
}
