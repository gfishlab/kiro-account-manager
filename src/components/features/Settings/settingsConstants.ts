export const AI_MODELS = [
  { value: 'auto', label: 'Auto (智能选择) - 1.0x', recommended: false },
  { value: 'claude-opus-4.8', label: 'Claude Opus 4.8 (1M) - 2.2x', recommended: true },
  { value: 'claude-opus-4.7', label: 'Claude Opus 4.7 (1M) - 2.2x', recommended: false },
  { value: 'claude-opus-4.6', label: 'Claude Opus 4.6 (1M) - 2.2x', recommended: false },
  { value: 'claude-sonnet-4.6', label: 'Claude Sonnet 4.6 (1M) - 1.3x', recommended: false },
  { value: 'claude-opus-4.5', label: 'Claude Opus 4.5 (200K) - 2.2x', recommended: false },
  { value: 'claude-sonnet-4.5', label: 'Claude Sonnet 4.5 (200K) - 1.3x', recommended: false },
  { value: 'claude-haiku-4.5', label: 'Claude Haiku 4.5 (200K) - 0.4x', recommended: false },
  { value: 'claude-sonnet-4', label: 'Claude Sonnet 4.0 (200K) - 1.3x', recommended: false },
  { value: 'deepseek-3.2', label: 'DeepSeek 3.2 (128K) - 0.25x', recommended: false },
  { value: 'minimax-m2.5', label: 'MiniMax M2.5 (200K) - 0.25x', recommended: false },
  { value: 'glm-5', label: 'GLM-5 (200K) - 0.5x', recommended: false },
  { value: 'minimax-m2.1', label: 'MiniMax M2.1 (200K) - 0.15x', recommended: false },
  { value: 'qwen3-coder-next', label: 'Qwen3 Coder Next (256K) - 0.05x', recommended: false },
]

export const NOTIFICATION_SETTINGS_FIELD_MAP = {
  'kiroAgent.notifications.agent.actionRequired': 'notifyActionRequired',
  'kiroAgent.notifications.agent.failure': 'notifyFailure',
  'kiroAgent.notifications.agent.success': 'notifySuccess',
  'kiroAgent.notifications.billing': 'notifyBilling'
}

export const buildThemeOptions = (t) => [
  { key: 'light', name: t('settings.light') || 'Light', iconName: 'Sun', color: 'from-blue-400 to-blue-600' },
  { key: 'dark', name: t('settings.dark') || 'Dark', iconName: 'Moon', color: 'from-gray-700 to-gray-900' },
  { key: 'purple', name: t('settings.purple') || 'Purple', iconName: 'Palette', color: 'from-purple-500 to-purple-700' },
  { key: 'green', name: t('settings.green') || 'Green', iconName: 'Palette', color: 'from-emerald-500 to-emerald-700' },
  { key: 'tech', name: t('settings.tech') || 'Tech Blue', iconName: 'Palette', color: 'from-blue-500 to-cyan-500' },
  { key: 'dark-one', name: t('settings.darkOne') || 'Dark One', iconName: 'Moon', color: 'from-slate-700 to-gray-900' },
  { key: 'business', name: t('settings.business') || 'Business', iconName: 'Palette', color: 'from-amber-500 to-yellow-600' },
  { key: 'sunset', name: t('settings.sunset') || 'Sunset', iconName: 'Palette', color: 'from-orange-400 to-red-500' },
  { key: 'ocean', name: t('settings.ocean') || 'Ocean', iconName: 'Palette', color: 'from-cyan-400 to-blue-500' },
  { key: 'rose', name: t('settings.rose') || 'Rose', iconName: 'Palette', color: 'from-pink-400 to-rose-500' },
  { key: 'aurora', name: t('settings.aurora') || 'Aurora', iconName: 'Palette', color: 'from-teal-400 to-emerald-500' },
  { key: 'midnight', name: t('settings.midnight') || 'Midnight', iconName: 'Moon', color: 'from-gray-900 via-yellow-700 to-black' },
  { key: 'forest', name: t('settings.forest') || 'Forest', iconName: 'Palette', color: 'from-green-600 to-emerald-900' },
  { key: 'sakura', name: t('settings.sakura') || 'Sakura', iconName: 'Palette', color: 'from-pink-200 to-rose-400' },
]
