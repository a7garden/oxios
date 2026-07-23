// System Agent model assignment types (ported from LobeHub)
// Lets users assign different models to different system tasks:
// topic naming, translation, compression, memory analysis, etc.

// ── Single system agent config ──

export interface SystemAgentItem {
  /** Model id in "provider/model" format. Empty = use default engine model. */
  model?: string
  /** Token cap for this task. */
  contextLimit?: number
  /** Override system prompt. */
  customPrompt?: string
  /** Toggle this system task. */
  enabled?: boolean
}

// ── All system agents ──

export interface SystemAgentConfig {
  /** Auto topic naming — names conversation topics. */
  topic?: SystemAgentItem
  /** AI image topic naming — names AI-generated image topics. */
  generationTopic?: SystemAgentItem
  /** Message translation. */
  translation?: SystemAgentItem
  /** Conversation history compression. */
  historyCompress?: SystemAgentItem
  /** Agent metadata generation (name, description, avatar, tags). */
  agentMeta?: SystemAgentItem
  /** Follow-up suggestion chips after each response. */
  followUpAction?: SystemAgentItem
  /** Input auto-complete (ghost text, like GitHub Copilot). */
  inputCompletion?: SystemAgentItem
  /** Prompt rewriting for better results. */
  promptRewrite?: SystemAgentItem
}

// ── Memory service models (separate group) ──

export interface MemoryServiceModelConfig {
  /** Analyze if conversation contains memory worth saving. */
  memoryAnalysis?: SystemAgentItem
  /** Embedding model for memory vector search. */
  embedding?: SystemAgentItem
  /** Write personalized memory summaries (persona). */
  personaWriter?: SystemAgentItem
}

// ── Full service model config ──

export interface ServiceModelConfig {
  /** Default model for new agents. */
  defaultAgent?: string
  systemAgents: SystemAgentConfig
  memoryModels: MemoryServiceModelConfig
}

// ── Defaults ──

export const DEFAULT_SYSTEM_AGENT: SystemAgentItem = {
  enabled: true,
  model: '', // empty = inherit default
}

export const DEFAULT_SERVICE_MODEL_CONFIG: ServiceModelConfig = {
  defaultAgent: '',
  systemAgents: {
    topic: { ...DEFAULT_SYSTEM_AGENT },
    generationTopic: { ...DEFAULT_SYSTEM_AGENT },
    translation: { ...DEFAULT_SYSTEM_AGENT },
    historyCompress: { ...DEFAULT_SYSTEM_AGENT },
    agentMeta: { ...DEFAULT_SYSTEM_AGENT },
    followUpAction: { ...DEFAULT_SYSTEM_AGENT, enabled: false },
    inputCompletion: { ...DEFAULT_SYSTEM_AGENT, enabled: false },
    promptRewrite: { ...DEFAULT_SYSTEM_AGENT, enabled: false },
  },
  memoryModels: {
    memoryAnalysis: { ...DEFAULT_SYSTEM_AGENT },
    embedding: { ...DEFAULT_SYSTEM_AGENT },
    personaWriter: { ...DEFAULT_SYSTEM_AGENT },
  },
}

// ── Metadata for UI rendering ──

export interface SystemAgentMeta {
  key: string
  label: string
  description: string
  icon: string
  group: 'system' | 'memory' | 'optional'
  supportsContextLimit?: boolean
}

export const SYSTEM_AGENT_METADATA: SystemAgentMeta[] = [
  // System agents
  {
    key: 'topic',
    label: 'Topic Auto-Naming',
    description: 'Model used for automatic conversation topic naming',
    icon: 'Tag',
    group: 'system',
  },
  {
    key: 'generationTopic',
    label: 'AI Image Topic Naming',
    description: 'Model used for AI-generated image topic naming',
    icon: 'Image',
    group: 'system',
  },
  {
    key: 'translation',
    label: 'Message Translation',
    description: 'Model used for translating message content',
    icon: 'Languages',
    group: 'system',
  },
  {
    key: 'historyCompress',
    label: 'History Compression',
    description: 'Model used for compressing conversation history',
    icon: 'Archive',
    group: 'system',
  },
  {
    key: 'agentMeta',
    label: 'Agent Info Generation',
    description: 'Model for generating agent name, description, avatar, tags',
    icon: 'Bot',
    group: 'system',
  },
  // Memory models
  {
    key: 'memoryAnalysis',
    label: 'Memory Analysis',
    description:
      'Determines if conversation contains memory and extracts identity, preferences, context, activity, experience',
    icon: 'Brain',
    group: 'memory',
  },
  {
    key: 'embedding',
    label: 'Memory Embedding',
    description: 'Embeds memory content for search. Context limit caps each embedding input.',
    icon: 'Box',
    group: 'memory',
    supportsContextLimit: true,
  },
  {
    key: 'personaWriter',
    label: 'Memory Persona Writer',
    description: 'Writes personalized memory summaries',
    icon: 'UserCircle',
    group: 'memory',
  },
  // Optional features
  {
    key: 'followUpAction',
    label: 'Follow-up Suggestions',
    description: 'Suggests one-click follow-up responses under each assistant message',
    icon: 'MessageCircle',
    group: 'optional',
  },
  {
    key: 'inputCompletion',
    label: 'Input Auto-complete',
    description: 'Ghost text suggestions while typing (like GitHub Copilot)',
    icon: 'Sparkles',
    group: 'optional',
  },
  {
    key: 'promptRewrite',
    label: 'Prompt Rewriting',
    description: 'Rewrites prompts for better results before sending',
    icon: 'Wand2',
    group: 'optional',
  },
]
