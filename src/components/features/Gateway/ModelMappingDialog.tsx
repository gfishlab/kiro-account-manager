import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Plus, Trash2, Zap } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'

// Kiro 内部模型格式（点号分隔）— 用于目标模型下拉
const TARGET_MODELS = [
  'claude-opus-4.7',
  'claude-opus-4.7-thinking',
  'claude-opus-4.6',
  'claude-opus-4.6-thinking',
  'claude-sonnet-4.6',
  'claude-sonnet-4.6-thinking',
  'claude-opus-4.5',
  'claude-opus-4.5-thinking',
  'claude-sonnet-4.5',
  'claude-sonnet-4.5-thinking',
  'claude-haiku-4.5',
  'claude-haiku-4.5-thinking',
  'claude-sonnet-4',
  'claude-sonnet-4-thinking',
  'deepseek-3.2',
  'minimax-m2.5',
  'glm-5',
  'qwen3-coder-next',
]

// 常见源模型名 — 用于源模型下拉
const SOURCE_MODELS = [
  // Claude（含 thinking 变体）
  'claude-opus-4.7',
  'claude-opus-4.7-thinking',
  'claude-opus-4.6',
  'claude-opus-4.6-thinking',
  'claude-sonnet-4.6',
  'claude-sonnet-4.6-thinking',
  'claude-opus-4.5',
  'claude-opus-4.5-thinking',
  'claude-sonnet-4.5',
  'claude-sonnet-4.5-thinking',
  'claude-haiku-4.5',
  'claude-haiku-4.5-thinking',
  'claude-sonnet-4',
  'claude-sonnet-4-thinking',
  // GPT 5.5 系列
  'gpt-5.5',
  'gpt-5.5-pro',
  'gpt-5.5-instant',
  // GPT 5.4 系列
  'gpt-5.4',
  'gpt-5.4-pro',
  'gpt-5.4-mini',
  // GPT 5.3 系列
  'gpt-5.3-codex',
  'gpt-5.3-codex-spark',
  'gpt-5.3-instant',
  // GPT 5.2 系列
  'gpt-5.2',
  'gpt-5.2-pro',
  'gpt-5.2-codex',
  // GPT 5.1 系列
  'gpt-5.1',
  'gpt-5.1-pro',
  'gpt-5.1-codex',
  'gpt-5.1-codex-max',
  'gpt-5.1-codex-mini',
  'gpt-5.1-instant',
]

// 预置 GPT/Codex → Claude 映射规则（5.5 ~ 5.1 系列）
const PRESET_RULES = [
  // GPT-5.5 系列 → Opus 4.7
  { source: 'gpt-5.5', target: 'claude-opus-4.7', name: 'GPT-5.5 → Opus 4.7' },
  { source: 'gpt-5.5-pro', target: 'claude-opus-4.7', name: 'GPT-5.5-pro → Opus 4.7' },
  { source: 'gpt-5.5-instant', target: 'claude-sonnet-4.6', name: 'GPT-5.5-instant → Sonnet 4.6' },
  // GPT-5.4 系列 → Opus/Sonnet 4.6
  { source: 'gpt-5.4', target: 'claude-opus-4.6', name: 'GPT-5.4 → Opus 4.6' },
  { source: 'gpt-5.4-pro', target: 'claude-opus-4.6', name: 'GPT-5.4-pro → Opus 4.6' },
  { source: 'gpt-5.4-mini', target: 'claude-sonnet-4.6', name: 'GPT-5.4-mini → Sonnet 4.6' },
  // GPT-5.3 系列 → Opus 4.5 / Sonnet 4.5
  { source: 'gpt-5.3-codex', target: 'claude-opus-4.5', name: 'GPT-5.3-codex → Opus 4.5' },
  { source: 'gpt-5.3-codex-spark', target: 'claude-sonnet-4.5', name: 'GPT-5.3-codex-spark → Sonnet 4.5' },
  { source: 'gpt-5.3-instant', target: 'claude-sonnet-4.5', name: 'GPT-5.3-instant → Sonnet 4.5' },
  // GPT-5.2 系列 → Opus 4.5
  { source: 'gpt-5.2', target: 'claude-opus-4.5', name: 'GPT-5.2 → Opus 4.5' },
  { source: 'gpt-5.2-pro', target: 'claude-opus-4.5', name: 'GPT-5.2-pro → Opus 4.5' },
  { source: 'gpt-5.2-codex', target: 'claude-opus-4.5', name: 'GPT-5.2-codex → Opus 4.5' },
  // GPT-5.1 系列 → Sonnet 4.5 / Haiku 4.5
  { source: 'gpt-5.1', target: 'claude-sonnet-4.5', name: 'GPT-5.1 → Sonnet 4.5' },
  { source: 'gpt-5.1-pro', target: 'claude-sonnet-4.5', name: 'GPT-5.1-pro → Sonnet 4.5' },
  { source: 'gpt-5.1-codex', target: 'claude-sonnet-4.5', name: 'GPT-5.1-codex → Sonnet 4.5' },
  { source: 'gpt-5.1-codex-max', target: 'claude-opus-4.5', name: 'GPT-5.1-codex-max → Opus 4.5' },
  { source: 'gpt-5.1-codex-mini', target: 'claude-haiku-4.5', name: 'GPT-5.1-codex-mini → Haiku 4.5' },
  { source: 'gpt-5.1-instant', target: 'claude-haiku-4.5', name: 'GPT-5.1-instant → Haiku 4.5' },
]

interface ModelMappingDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  modelMappings: any[]
  setField: (key: string, value: any) => void
  onSave?: () => void
  accountId?: string | null
}

function ModelMappingDialog({ open, onOpenChange, modelMappings, setField, onSave, accountId }: ModelMappingDialogProps) {
  const rules = modelMappings || []

  // 实时拉取账号可用模型（走后端缓存），claude 系列补 -thinking 变体，与静态列表并集去重作兜底
  const [liveModels, setLiveModels] = useState<string[]>([])
  useEffect(() => {
    if (!open || !accountId) return
    invoke<any>('list_available_models', { id: accountId })
      .then(res => {
        const ids = (Array.isArray(res?.availableModels) ? res.availableModels : [])
          .map((m: any) => m?.modelId)
          .filter((id: any): id is string => typeof id === 'string' && id.length > 0)
        setLiveModels(ids.flatMap((id: string) =>
          id.startsWith('claude-') && !id.endsWith('-thinking') ? [id, `${id}-thinking`] : [id]
        ))
      })
      .catch(() => setLiveModels([]))
  }, [open, accountId])

  const sourceOptions = Array.from(new Set([...liveModels, ...SOURCE_MODELS]))
  const targetOptions = Array.from(new Set([...liveModels, ...TARGET_MODELS]))

  const handleToggle = (idx: number, checked: boolean) => {
    const updated = [...rules]
    updated[idx] = { ...updated[idx], enabled: checked }
    setField('modelMappings', updated)
  }

  const handleDelete = (idx: number) => {
    setField('modelMappings', rules.filter((_: any, i: number) => i !== idx))
  }

  const handleAdd = () => {
    const sourceEl = document.getElementById('dialog-mapping-source') as HTMLInputElement
    const targetEl = document.getElementById('dialog-mapping-target') as HTMLInputElement
    if (!sourceEl?.value?.trim() || !targetEl?.value?.trim()) return

    const newRule = {
      id: crypto.randomUUID(),
      name: `${sourceEl.value.trim()} → ${targetEl.value.trim()}`,
      enabled: true,
      ruleType: 'replace',
      sourceModel: sourceEl.value.trim(),
      targetModels: [targetEl.value.trim()],
      weights: []
    }
    setField('modelMappings', [...rules, newRule])
    sourceEl.value = ''
    targetEl.value = ''
  }

  const handlePreset = () => {
    const existingSources = new Set(rules.map((r: any) => r.sourceModel))
    const newRules = PRESET_RULES
      .filter(p => !existingSources.has(p.source))
      .map(p => ({
        id: crypto.randomUUID(),
        name: p.name,
        enabled: true,
        ruleType: 'replace',
        sourceModel: p.source,
        targetModels: [p.target],
        weights: []
      }))
    if (newRules.length > 0) {
      setField('modelMappings', [...rules, ...newRules])
    }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => { onOpenChange(v); if (!v && onSave) onSave() }}>
      <DialogContent className="sm:max-w-[560px]">
        <DialogHeader>
          <DialogTitle>模型映射规则</DialogTitle>
          <DialogDescription>
            客户端请求的模型名会根据规则映射到 Kiro 内部模型
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-4 mt-2">
          {/* 规则列表 */}
          <div className="border rounded-lg overflow-hidden max-h-[300px] overflow-y-auto">
            {rules.length === 0 ? (
              <div className="p-6 text-center text-sm text-muted-foreground">
                暂无规则
              </div>
            ) : (
              rules.map((rule: any, idx: number) => (
                <div key={rule.id} className={`flex items-center gap-2 p-3 border-b last:border-b-0 hover:bg-muted/30 ${!rule.enabled ? 'opacity-50' : ''}`}>
                  <Switch
                    size="sm"
                    checked={rule.enabled}
                    onCheckedChange={(checked) => handleToggle(idx, checked)}
                  />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-1.5">
                      <span className="text-sm font-medium truncate">{rule.name || rule.sourceModel}</span>
                      <Badge variant="outline" className="text-[10px] px-1.5 py-0">
                        {rule.ruleType === 'replace' ? '替换' : rule.ruleType === 'alias' ? '别名' : '负载均衡'}
                      </Badge>
                    </div>
                    <div className="text-xs text-muted-foreground font-mono truncate mt-0.5">
                      {rule.sourceModel} → {rule.targetModels.join(', ')}
                    </div>
                  </div>
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-7 w-7 p-0 text-red-500 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-950/20"
                    onClick={() => handleDelete(idx)}
                  >
                    <Trash2 size={14} />
                  </Button>
                </div>
              ))
            )}
          </div>

          {/* 添加新规则 */}
          <div className="space-y-2 p-3 border rounded-lg bg-muted/10">
            <div className="text-xs font-medium text-muted-foreground">添加新规则</div>
            <div className="grid grid-cols-2 gap-2">
              <div className="relative">
                <Input placeholder="源模型名" className="text-xs" id="dialog-mapping-source" list="model-list-source" />
                <datalist id="model-list-source">
                  {sourceOptions.map(m => <option key={m} value={m} />)}
                </datalist>
              </div>
              <div className="relative">
                <Input placeholder="目标模型名" className="text-xs" id="dialog-mapping-target" list="model-list-target" />
                <datalist id="model-list-target">
                  {targetOptions.map(m => <option key={m} value={m} />)}
                </datalist>
              </div>
            </div>
            <div className="flex gap-2">
              <Select defaultValue="replace">
                <SelectTrigger className="text-xs flex-1" id="dialog-mapping-type">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="replace">替换 (replace)</SelectItem>
                  <SelectItem value="alias">别名 (alias)</SelectItem>
                </SelectContent>
              </Select>
              <Button size="sm" className="text-xs" onClick={handleAdd}>
                <Plus size={14} className="mr-1" />
                添加
              </Button>
            </div>
          </div>

          {/* 预置规则 */}
          <div className="flex items-center justify-between pt-1">
            <div className="text-xs text-muted-foreground">快速添加 OpenAI/Codex 兼容映射</div>
            <Button size="sm" variant="outline" className="h-7 text-xs" onClick={handlePreset}>
              <Zap size={12} className="mr-1" />
              预置 GPT 映射
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

export default ModelMappingDialog
