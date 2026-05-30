// Seed Phase 1 Types

export type OuroborosPhase = 'interview' | 'seed' | 'execute' | 'evaluate' | 'evolve'

export interface SeedDetail {
  id: string
  goal: string
  constraints: string[]
  acceptance_criteria: string[]
  ontology: SeedEntity[]
  phase_reached: OuroborosPhase
  generation: number
  parent_seed_id?: string
  created_at: string
  evaluation?: EvaluationResult
  execution_result?: {
    success: boolean
    steps_completed: number
    duration_ms: number
    error?: string
  }
  [key: string]: unknown
}

export interface SeedEntity {
  name: string
  kind: string
  description: string
}

export interface EvaluationResult {
  mechanical: {
    passed: boolean
    details: string
  }
  semantic: {
    passed: boolean
    score: number
    details: string
  }
  consensus?: {
    agreed: boolean
    details: string
  }
  score: number
  all_passed: boolean
}

export interface EvolutionEntry {
  id: string
  generation: number
  goal: string
  parent_id?: string
  score?: number
  passed?: boolean
}

export interface LinkedAgent {
  id: string
  name: string
  status: string
  steps_completed: number
  created_at: string
}
