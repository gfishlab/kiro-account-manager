// 与后端 src-tauri/src/clients/http_client.rs::SUPPORTED_KIRO_REGIONS 对齐。
// 改这里时同步改后端那份；GatewayConfig.tsx 的 <SelectItem> 列表也要保持一致。
const ALLOWED_REGIONS = [
  'us-east-1',
  'us-east-2',
  'us-west-1',
  'us-west-2',
  'eu-west-1',
  'eu-west-2',
  'eu-west-3',
  'eu-central-1',
  'eu-central-2',
  'eu-north-1',
  'eu-south-1',
  'eu-south-2',
  'ap-northeast-1',
  'ap-northeast-2',
  'ap-northeast-3',
  'ap-southeast-1',
  'ap-southeast-2',
  'ap-southeast-3',
  'ap-southeast-4',
  'ap-southeast-5',
  'ap-southeast-7',
  'ap-south-1',
  'ap-south-2',
  'ap-east-1',
  'ca-central-1',
  'ca-west-1',
  'sa-east-1',
  'me-south-1',
  'me-central-1',
  'il-central-1',
  'mx-central-1',
  'af-south-1',
  'us-gov-west-1',
  'us-gov-east-1',
  'cn-north-1',
  'cn-northwest-1',
] as const

export const parseAllowedIps = (value: string | string[]): string[] => {
  if (Array.isArray(value)) return value
  return String(value || '')
    .split(/[\n,]+/)
    .map(item => item.trim())
    .filter(Boolean)
}

export const parseClientApiKeys = (value: string | string[]): string[] => {
  if (Array.isArray(value)) return value
  return String(value || '')
    .split(/[\n,]+/)
    .map(item => item.trim())
    .filter(Boolean)
    .filter((item, index, items) => items.indexOf(item) === index)
}

export const getPrimaryClientApiKey = (value: string | string[]): string =>
  parseClientApiKeys(value)[0] || ''

const isValidIpv4Address = (value: string): boolean => {
  if (!/^\d{1,3}(\.\d{1,3}){3}$/.test(value)) {
    return false
  }

  return value
    .split('.')
    .every(part => {
      const number = Number(part)
      return Number.isInteger(number) && number >= 0 && number <= 255
    })
}

const isValidIpv6Address = (value: string): boolean => {
  try {
    const parsed = new URL(`http://[${value}]/`)
    return parsed.hostname === value
  } catch {
    return false
  }
}

const isValidGatewayHost = (value: string): boolean => {
  const host = String(value || '').trim()
  if (!host) {
    return false
  }

  if (host === 'localhost' || host === '0.0.0.0' || host === '::' || host === '::1') {
    return true
  }

  return isValidIpv4Address(host) || isValidIpv6Address(host)
}

const isValidAllowlistEntry = (value: string): boolean => {
  const entry = String(value || '').trim()
  if (!entry) {
    return false
  }

  if (isValidIpv4Address(entry) || isValidIpv6Address(entry)) {
    return true
  }

  const [host, prefix, ...rest] = entry.split('/')
  if (!host || !prefix || rest.length) {
    return false
  }

  if (!/^\d+$/.test(prefix)) {
    return false
  }

  const prefixNumber = Number(prefix)
  if (isValidIpv4Address(host)) {
    return prefixNumber >= 0 && prefixNumber <= 32
  }
  if (isValidIpv6Address(host)) {
    return prefixNumber >= 0 && prefixNumber <= 128
  }

  return false
}

export const buildGatewayConnectHost = (host: string, localOnly: boolean): string => {
  const value = String(host || '').trim()
  if (!value) {
    return '127.0.0.1'
  }
  if (value === '0.0.0.0' || value === '::') {
    return localOnly ? '127.0.0.1' : 'localhost'
  }

  return value
}

export const buildGatewayBaseUrl = (host: string, port: number, localOnly: boolean): string => {
  const connectHost = buildGatewayConnectHost(host, localOnly)
  const needsBrackets = connectHost.includes(':') && !connectHost.startsWith('[')
  const normalizedHost = needsBrackets ? `[${connectHost}]` : connectHost
  return `http://${normalizedHost}:${Number(port) || 8765}`
}

export const applyGatewayLocalOnlyChange = (
  config: any,
  nextLocalOnly: boolean,
  createApiKey: () => string
): any => {
  const nextConfig = {
    ...config,
    localOnly: !!nextLocalOnly
  }

  if (!nextLocalOnly && !parseClientApiKeys(config?.clientApiKeysText || config?.apiKey).length) {
    const generatedKey = createApiKey()
    return {
      ...nextConfig,
      apiKey: generatedKey,
      clientApiKeysText: generatedKey
    }
  }

  return nextConfig
}

export const formatGatewayTimestamp = (date = new Date()): string => {
  const pad = (value: number) => String(value).padStart(2, '0')

  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}

export const createGatewayFieldErrors = (config: any) => {
  const errors: any = {}
  const host = String(config?.host || '').trim()
  const port = Number(config?.port)
  const region = String(config?.region || '').trim()
  const accountMode = String(config?.accountMode || 'single').trim()
  const localOnly = config?.localOnly ?? true
  const allowedIps = parseAllowedIps(config?.allowedIpsText)
  const clientApiKeys = parseClientApiKeys(config?.clientApiKeysText || config?.apiKey)

  if (!host) {
    errors.host = '监听地址不能为空'
  } else if (!isValidGatewayHost(host)) {
    errors.host = '监听地址必须是 localhost、IPv4 或 IPv6 地址'
  }

  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    errors.port = '端口必须在 1-65535 之间'
  }

  if (!region) {
    errors.region = 'region 不能为空'
  } else if (!(ALLOWED_REGIONS as readonly string[]).includes(region)) {
    errors.region = `region 不受支持: ${region}`
  }

  if (!['single', 'group', 'pool'].includes(accountMode)) {
    errors.accountMode = 'accountMode 必须是 single/group/pool'
  } else if (accountMode === 'single' && !String(config?.accountId || '').trim()) {
    errors.accountId = 'single 模式必须选择账号'
  } else if (accountMode === 'group' && !String(config?.groupId || '').trim()) {
    errors.groupId = 'group 模式必须选择分组'
  }

  if (!clientApiKeys.length) {
    errors.clientApiKeysText = '必须至少填写一个客户端 API Key'
  }

  if (!localOnly && !allowedIps.length) {
    errors.allowedIpsText = '允许远程访问时必须至少配置一个白名单来源 IP'
  }

  const invalidAllowlistEntry = allowedIps.find(entry => !isValidAllowlistEntry(entry))
  if (invalidAllowlistEntry) {
    errors.allowedIpsText = `白名单条目无效: ${invalidAllowlistEntry}`
  }

  return errors
}

export const redactGatewayApiKey = (apiKey: string): string => {
  const value = String(apiKey || '').trim()
  if (!value) {
    return 'sk-your-gateway-api-key'
  }
  if (value.length <= 8) {
    return `${value.slice(0, 3)}***`
  }
  return `${value.slice(0, 6)}...${value.slice(-4)}`
}

interface ErrorHistoryEntry {
  message: string
  firstSeenAt: string
  lastSeenAt: string
  count: number
}

export const mergeErrorHistory = (
  history: ErrorHistoryEntry[],
  message: string,
  seenAt: string,
  limit = 8
): ErrorHistoryEntry[] => {
  const normalizedMessage = String(message || '').trim()
  if (!normalizedMessage) {
    return history
  }

  const existingIndex = history.findIndex(item => item.message === normalizedMessage)
  if (existingIndex >= 0) {
    const next = [...history]
    const existing = next[existingIndex]
    next[existingIndex] = {
      ...existing,
      count: existing.count + 1,
      lastSeenAt: seenAt
    }
    return next
  }

  return [
    {
      message: normalizedMessage,
      firstSeenAt: seenAt,
      lastSeenAt: seenAt,
      count: 1
    },
    ...history,
  ].slice(0, limit)
}

interface ClientSamples {
  anthropic: { env: string }
  openai: { env: string; curl: string }
  openaiChat: { env: string; curl: string }
  claudeCode: { config: string; apiKey: string }
  codex: { config: string; apiKey: string }
}

export const buildClientSamples = (baseUrl: string, apiKey: string | string[]): ClientSamples => {
  const safeKey = redactGatewayApiKey(getPrimaryClientApiKey(apiKey))
  const realKey = getPrimaryClientApiKey(apiKey)
  const anthropicEnv = `ANTHROPIC_BASE_URL=${baseUrl}\nANTHROPIC_API_KEY=${safeKey}`
  const openaiEnv = `OPENAI_BASE_URL=${baseUrl}\nOPENAI_API_KEY=${safeKey}`
  const openaiResponsesCurl = [
    `curl ${baseUrl}/v1/responses \\`,
    '  -H "Content-Type: application/json" \\',
    `  -H "Authorization: Bearer ${safeKey}" \\`,
    '  -d "{\\"model\\":\\"claude-sonnet-4-5-20250929\\",\\"input\\":[{\\"role\\":\\"user\\",\\"content\\":[{\\"type\\":\\"input_text\\",\\"text\\":\\"hello\\"}]}]}"',
  ].join('\n')
  const openaiChatCurl = [
    `curl ${baseUrl}/v1/chat/completions \\`,
    '  -H "Content-Type: application/json" \\',
    `  -H "Authorization: Bearer ${safeKey}" \\`,
    '  -d "{\\"model\\":\\"claude-sonnet-4-5-20250929\\",\\"messages\\":[{\\"role\\":\\"user\\",\\"content\\":\\"hello\\"}]}"',
  ].join('\n')

  // Claude Code 配置（~/.claude/settings.json）
  const claudeCodeConfig = [
    '{',
    '  "env": {',
    `    "ANTHROPIC_BASE_URL": "${baseUrl}",`,
    `    "ANTHROPIC_AUTH_TOKEN": "${safeKey}"`,
    '  }',
    '}',
  ].join('\n')

  // Codex CLI 配置（~/.codex/config.toml + ~/.codex/auth.json）
  const codexConfig = [
    '# ~/.codex/config.toml',
    'model_provider = "custom"',
    'model = "claude-sonnet-4-5-20250929"',
    '',
    '[model_providers.custom]',
    'name = "custom"',
    `base_url = "${baseUrl}"`,
    'wire_api = "responses"',
    'requires_openai_auth = true',
    '',
    '# ~/.codex/auth.json',
    '{',
    `  "OPENAI_API_KEY": "${safeKey}"`,
    '}',
  ].join('\n')

  return {
    anthropic: {
      env: anthropicEnv
    },
    openai: {
      env: openaiEnv,
      curl: openaiResponsesCurl
    },
    openaiChat: {
      env: openaiEnv,
      curl: openaiChatCurl
    },
    claudeCode: {
      config: claudeCodeConfig,
      apiKey: realKey
    },
    codex: {
      config: codexConfig,
      apiKey: realKey
    }
  }
}

export const formatGatewayAccountOptionLabel = (account: any): string => {
  const email = String(account?.email || '').trim()
  const userId = String(account?.userId || '').trim()
  return email || userId || '未知账号'
}

export const buildGatewayStatusSummary = ({ config, status, errorHistory, lastStatusSyncAt }: any) => {
  const mode = config?.accountMode || 'single'
  const strategy = config?.strategy || 'round_robin'
  const listenHost = (status?.running ? status?.host : null) || config?.host || status?.host || '127.0.0.1'
  const listenPort = (status?.running ? status?.port : null) || config?.port || status?.port || 8765
  const errorCount = Array.isArray(errorHistory) ? errorHistory.length : 0
  const errorHits = Array.isArray(errorHistory) ? errorHistory.reduce((sum, item) => sum + Number(item.count || 0), 0) : 0
  return {
    listen: `http://${listenHost}:${listenPort}`,
    requests: String(status?.requestCount || 0),
    region: config?.region || 'us-east-1',
    logLevel: config?.logLevel || 'debug',
    sync: lastStatusSyncAt || '-',
    routing: `${mode} / ${strategy}`,
    exposure: config?.localOnly ? '仅本机' : '允许远程',
    errorCount: errorCount
  }
}
export const buildGatewayRoutingSummary = ({ config, counts, selectedLabels = {} }: any) => {
  const mode = config?.accountMode || 'single'
  const inventorySummary = `账号 ${counts?.accounts || 0} 个 / 分组 ${counts?.groups || 0} 个`

  if (mode === 'single') {
    return {
      modeLabel: '指定单账号',
      modeDescription: '反代会固定使用一个账号，适合调试或绑定到单一租户场景。',
      selectionLabel: '当前账号',
      selectionValue: selectedLabels.single || '未选择账号',
      inventorySummary,
      strategySummary: '固定账号，不参与轮换'}
  }

  if (mode === 'group') {
    return {
      modeLabel: '按分组账号池',
      modeDescription: '先锁定账号分组，再按策略和阈值从该分组中挑选可用账号。',
      selectionLabel: '当前分组',
      selectionValue: selectedLabels.group || '未选择分组',
      inventorySummary,
      strategySummary: `策略 ${config?.strategy || 'round_robin'} / 阈值 ${Number(config?.threshold) || 90}%`}
  }

  if (mode === 'pool') {
    return {
      modeLabel: '账号管理池',
      modeDescription: '使用所有可用账号，按策略和阈值自动选择，不限制分组。',
      selectionLabel: '账号范围',
      selectionValue: '所有可用账号',
      inventorySummary,
      strategySummary: `策略 ${config?.strategy || 'round_robin'} / 阈值 ${Number(config?.threshold) || 90}%`}
  }

  return {
    modeLabel: '按分组账号池',
    modeDescription: '先锁定账号分组，再按策略和阈值从该分组中挑选可用账号。',
    selectionLabel: '当前分组',
    selectionValue: selectedLabels.group || '未选择分组',
    inventorySummary,
    strategySummary: `策略 ${config?.strategy || 'round_robin'} / 阈值 ${Number(config?.threshold) || 90}%`}
}

export const buildGatewayActionSummary = ({
  running,
  isDirty,
  hasUnsavedChanges,
  hasRuntimeChanges,
  hasFieldErrors}: any) => {
  const unsavedChanges = hasUnsavedChanges ?? isDirty ?? false
  const runtimeChanges = hasRuntimeChanges ?? (running && unsavedChanges)

  if (hasFieldErrors) {
    return {
      tone: 'red',
      title: '先修正配置错误',
      description: '当前表单存在无效配置，保存、启动和重启都会被拦截，先修正标红字段。'}
  }

  if (running && unsavedChanges && runtimeChanges) {
    return {
      tone: 'yellow',
      title: '配置已变更，重启后生效',
      description: '反代仍按已启动时的配置运行。先保存，再执行重启反代，才能让新配置生效。'}
  }

  if (running && unsavedChanges) {
    return {
      tone: 'blue',
      title: '当前运行配置尚未保存',
      description: '当前页面配置已经用于运行反代，但还没有写回配置文件；如需保留下次启动沿用，请保存配置。'}
  }

  if (running) {
    return {
      tone: 'teal',
      title: '反代运行中',
      description: '当前配置与已保存状态一致；如需中断流量可直接停止反代。'}
  }

  if (unsavedChanges) {
    return {
      tone: 'blue',
      title: '可按当前配置直接启动',
      description: '启动反代会使用当前表单里的配置；如果希望下次应用启动也沿用这些设置，先点保存配置。'}
  }

  return {
    tone: 'blue',
    title: '反代当前未启动',
    description: '可以直接启动现有配置，或先调整表单后再启动。'}
}

export const buildGatewaySecuritySummary = ({ config }: any) => {
  const allowedIpsCount = parseAllowedIps(config?.allowedIpsText).length
  const clientApiKeys = parseClientApiKeys(config?.clientApiKeysText || config?.apiKey)

  return {
    exposureLabel: config?.localOnly ? '仅本机访问' : '允许远程访问',
    allowedIpsCount,
    apiKeyState: clientApiKeys.length
      ? `已配置 ${clientApiKeys.length} 个客户端 Key`
      : '未配置客户端 Key',
    logLevel: config?.logLevel || 'debug'}
}

export const buildGatewayIntegrationSummary = ({ baseUrl, apiKey, clientApiKeysText, logDir, errorHistory }: any) => {
  const clientApiKeys = parseClientApiKeys(clientApiKeysText || apiKey)
  const safeKey = redactGatewayApiKey(clientApiKeys[0] || '')
  const errorCount = Array.isArray(errorHistory) ? errorHistory.length : 0
  const errorHits = Array.isArray(errorHistory) ? errorHistory.reduce((sum, item) => sum + Number(item.count || 0), 0) : 0

  return {
    endpointLabel: baseUrl,
    authLabel: clientApiKeys.length > 1 ? `Bearer ${safeKey}（共 ${clientApiKeys.length} 个 Key）` : `Bearer ${safeKey}`,
    logDirState: String(logDir || '').trim() ? '日志目录已定位' : '日志目录未获取',
    errorDigest: `${errorCount} 条错误 / ${errorHits} 次命中`}
}

export const formatGatewayRequestDuration = (durationMs: number): string => {
  const duration = Number(durationMs) || 0
  if (duration < 1000) {
    return `${duration} ms`
  }
  return `${(duration / 1000).toFixed(duration >= 10_000 ? 0 : 2)} s`
}

export const getGatewayRequestOutcomeColor = (outcome: string): string => {
  if (outcome === 'success') return 'teal'
  if (outcome === 'stream') return 'blue'
  if (outcome === 'error') return 'red'
  return 'gray'
}

export const buildGatewayRequestLogSummary = (entries: any) => {
  const logs = Array.isArray(entries) ? entries : []
  const errors = logs.filter(item => item?.outcome === 'error').length
  const streaming = logs.filter(item => item?.outcome === 'stream').length
  const success = logs.filter(item => item?.outcome === 'success').length
  const maxDuration = logs.reduce((max, item) => Math.max(max, Number(item?.durationMs || 0)), 0)
  const latestOccurredAt = logs[0]?.occurredAt || '-'

  // Prompt Caching 统计
  let totalInputTokens = 0
  let totalOutputTokens = 0
  let totalCacheReadTokens = 0
  let totalCacheCreationTokens = 0
  let requestsWithCache = 0

  logs.forEach(item => {
    const inputTokens = Number(item?.inputTokens || 0)
    const outputTokens = Number(item?.outputTokens || 0)
    const cacheReadTokens = Number(item?.cacheReadInputTokens || 0)
    const cacheCreationTokens = Number(item?.cacheCreationInputTokens || 0)

    totalInputTokens += inputTokens
    totalOutputTokens += outputTokens
    totalCacheReadTokens += cacheReadTokens
    totalCacheCreationTokens += cacheCreationTokens

    if (cacheReadTokens > 0 || cacheCreationTokens > 0) {
      requestsWithCache++
    }
  })

  const total = logs.length
  const successRateLabel = total > 0 ? `${((success / total) * 100).toFixed(1)}%` : '0%'
  const errorRateLabel = total > 0 ? `${((errors / total) * 100).toFixed(1)}%` : '0%'

  // 计算缓存命中率
  const cacheHitRate = total > 0
    ? Math.round((requestsWithCache / total) * 100)
    : 0

  // 计算节省成本百分比（缓存读取成本是输入成本的 10%）
  const totalCacheableTokens = totalCacheReadTokens + totalCacheCreationTokens
  const costSavings = totalCacheableTokens > 0
    ? Math.round((totalCacheReadTokens / totalCacheableTokens) * 90)
    : 0

  return {
    total,
    errors,
    streaming,
    success,
    successRateLabel,
    errorRateLabel,
    maxDurationLabel: formatGatewayRequestDuration(maxDuration),
    latestOccurredAt,
    // Prompt Caching 统计
    totalInputTokens,
    totalOutputTokens,
    totalCacheReadTokens,
    totalCacheCreationTokens,
    requestsWithCache,
    cacheHitRate: `${cacheHitRate.toFixed(1)}%`,
    costSavings: `${costSavings.toFixed(2)}`
  }
}
interface MetricEntry {
  label: string
  count: number
  percent: string
}

const buildTopEntries = (values: Record<string, number>, total: number, limit = 5): MetricEntry[] =>
  Object.entries(values)
    .sort((left, right) => {
      if (right[1] !== left[1]) {
        return right[1] - left[1]
      }
      return left[0].localeCompare(right[0], 'zh-CN')
    })
    .slice(0, limit)
    .map(([label, count]) => ({
      label,
      count,
      percent: total > 0 ? `${Math.round((count / total) * 100)}%` : '0%'
    }))

export const buildGatewayMetricsSummary = (entries: any) => {
  const logs = Array.isArray(entries) ? entries : []
  const total = logs.length

  if (total === 0) {
    return {
      total: 0,
      avgDurationLabel: '0 ms',
      successRateLabel: '0%',
      errorRateLabel: '0%',
      uniqueModels: 0,
      uniqueUpstreams: 0,
      topModels: [],
      topUpstreams: [],
      topStatuses: [],
      topEndpoints: [],
      topRegions: []
    }
  }

  const outcomeCounts = { success: 0, stream: 0, error: 0, other: 0 }
  const modelCounts: Record<string, number> = {}
  const upstreamCounts: Record<string, number> = {}
  const statusCounts: Record<string, number> = {}
  const endpointCounts: Record<string, number> = {}
  const regionCounts: Record<string, number> = {}
  let totalDuration = 0

  logs.forEach(item => {
    totalDuration += Number(item?.durationMs || 0)

    const outcome = String(item?.outcome || 'other')
    if (outcomeCounts[outcome] !== undefined) {
      outcomeCounts[outcome] += 1
    } else {
      outcomeCounts.other += 1
    }

    const model = String(item?.model || '未记录模型').trim() || '未记录模型'
    modelCounts[model] = (modelCounts[model] || 0) + 1

    const upstream = String(item?.upstreamSource || '未解析上游来源').trim() || '未解析上游来源'
    upstreamCounts[upstream] = (upstreamCounts[upstream] || 0) + 1

    const status = String(item?.statusCode || 0)
    statusCounts[status] = (statusCounts[status] || 0) + 1

    const endpoint = String(item?.endpoint || '-')
    endpointCounts[endpoint] = (endpointCounts[endpoint] || 0) + 1

    const region = String(item?.region || '-')
    regionCounts[region] = (regionCounts[region] || 0) + 1
  })

  const avgDuration = Math.round(totalDuration / total)

  return {
    total,
    avgDurationLabel: formatGatewayRequestDuration(avgDuration),
    successRateLabel: `${Math.round((outcomeCounts.success / total) * 100)}%`,
    errorRateLabel: `${Math.round((outcomeCounts.error / total) * 100)}%`,
    uniqueModels: Object.keys(modelCounts).length,
    uniqueUpstreams: Object.keys(upstreamCounts).length,
    topModels: buildTopEntries(modelCounts, total),
    topUpstreams: buildTopEntries(upstreamCounts, total),
    topStatuses: buildTopEntries(statusCounts, total),
    topEndpoints: buildTopEntries(endpointCounts, total),
    topRegions: buildTopEntries(regionCounts, total)
  }
}

const stringifyGatewayRequestLog = (entry: any): string => {
  if (!entry || typeof entry !== 'object') {
    return ''
  }

  return [
    entry.endpoint,
    entry.outcome,
    entry.model,
    entry.region,
    entry.clientIp,
    entry.upstreamSource,
    entry.error,
    entry.requestBody,
    entry.responseBody,
    entry.statusCode,
    entry.requestIndex,
    entry.occurredAt,
  ]
    .filter(value => value !== undefined && value !== null)
    .join(' ')
    .toLowerCase()
}

interface FilterOptions {
  outcome?: string
  query?: string
}

export const filterGatewayRequestLogs = (entries: any[], options: FilterOptions = {}): any[] => {
  const { outcome = 'all', query = '' } = options
  const logs = Array.isArray(entries) ? entries : []
  const normalizedOutcome = String(outcome || 'all').trim()
  const normalizedQuery = String(query || '').trim().toLowerCase()

  return logs.filter(entry => {
    if (normalizedOutcome !== 'all' && entry?.outcome !== normalizedOutcome) {
      return false
    }

    if (!normalizedQuery) {
      return true
    }

    return stringifyGatewayRequestLog(entry).includes(normalizedQuery)
  })
}

