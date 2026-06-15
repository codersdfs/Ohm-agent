import { useEffect, useState, useCallback } from 'react'
import { useChatStore } from '../stores/chatStore'
import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { StructuredPlan, PlanStep, PlanGeneratedPayload } from '../lib/tauri'
import { getPlan, approvePlan, generatePlan } from '../lib/tauri'

export function PlanPanel() {
  const [plan, setPlan] = useState<StructuredPlan | null>(null)
  const [isGenerating, setIsGenerating] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const lastTask = useChatStore((s) => s.messages.filter(m => m.role === 'user').at(-1)?.content ?? '')
  const [unlisten, setUnlisten] = useState<UnlistenFn | null>(null)

  useEffect(() => {
    getPlan().then(setPlan).catch(() => {})

    listen<PlanGeneratedPayload>('plan-generated', (event) => {
      setPlan(event.payload.plan)
      setIsGenerating(false)
    }).then(setUnlisten)

    return () => { unlisten?.() }
  }, [])

  const handleGenerate = useCallback(async () => {
    if (!lastTask.trim()) return
    setIsGenerating(true)
    setError(null)
    try {
      await generatePlan(lastTask)
    } catch (e) {
      setError(String(e))
      setIsGenerating(false)
    }
  }, [lastTask])

  const handleApprove = useCallback(async () => {
    try {
      await approvePlan()
    } catch (e) {
      setError(String(e))
    }
  }, [])

  const actionColors: Record<string, string> = {
    create: 'text-green',
    modify: 'text-blue',
    delete: 'text-red',
    refactor: 'text-yellow',
    test: 'text-omega-300',
  }

  function StepCard({ step }: { step: PlanStep }) {
    return (
      <div className="bg-omega-800 border border-omega-700 rounded-lg p-3">
        <div className="flex items-center justify-between mb-1.5">
          <div className="flex items-center gap-2">
            <span className="text-xs font-mono text-omega-500">#{step.id}</span>
            <span className={`text-xs font-mono font-medium ${actionColors[step.action] || 'text-omega-300'}`}>
              {step.action.toUpperCase()}
            </span>
          </div>
          {step.estimated_lines && (
            <span className="text-[10px] text-omega-500">~{step.estimated_lines} lines</span>
          )}
        </div>
        <p className="text-xs text-omega-200 mb-1">{step.description}</p>
        {step.file_path && (
          <div className="text-[10px] font-mono text-omega-500">{step.file_path}</div>
        )}
        {step.dependencies.length > 0 && (
          <div className="text-[10px] text-omega-500 mt-1">
            Depends on: #{step.dependencies.join(', #')}
          </div>
        )}
      </div>
    )
  }

  return (
    <div className="flex-1 overflow-y-auto p-4 space-y-4">
      <h2 className="text-sm font-semibold text-omega-200">Plan</h2>

      {!plan && !isGenerating && (
        <div className="space-y-2">
          <p className="text-xs text-omega-500">
            {lastTask
              ? `Generate a plan based on: "${lastTask.slice(0, 100)}${lastTask.length > 100 ? '...' : ''}"`
              : 'Send a message in Chat to describe what you want to build.'}
          </p>
          {lastTask && (
            <button
              onClick={handleGenerate}
              className="bg-accent hover:bg-accent-hover text-white text-xs px-4 py-2 rounded-lg transition-colors"
            >
              Generate Plan
            </button>
          )}
        </div>
      )}

      {isGenerating && (
        <div className="bg-omega-800 rounded-lg p-4">
          <div className="flex items-center gap-2 text-xs text-omega-400">
            <span className="w-2 h-2 rounded-full bg-accent animate-pulse" />
            Generating plan...
          </div>
        </div>
      )}

      {error && (
        <div className="bg-red-900/20 border border-red-700/30 rounded-lg p-3 text-xs text-red">
          {error}
        </div>
      )}

      {plan && (
        <div className="space-y-3">
          <div className="bg-omega-800 border border-omega-700 rounded-lg p-4">
            <div className="text-xs font-semibold text-omega-200 mb-2">{plan.task_summary}</div>
            <div className="flex flex-wrap gap-2 text-[10px]">
              <span className="bg-omega-700 text-omega-300 px-2 py-0.5 rounded">{plan.language}</span>
              <span className={`px-2 py-0.5 rounded ${
                plan.estimated_complexity === 'low' ? 'bg-green/20 text-green'
                : plan.estimated_complexity === 'medium' ? 'bg-yellow/20 text-yellow'
                : 'bg-red/20 text-red'
              }`}>
                {plan.estimated_complexity} complexity
              </span>
              <span className={`px-2 py-0.5 rounded ${
                plan.risk_level === 'low' ? 'bg-green/20 text-green'
                : plan.risk_level === 'medium' ? 'bg-yellow/20 text-yellow'
                : 'bg-red/20 text-red'
              }`}>
                {plan.risk_level} risk
              </span>
              <span className="bg-omega-700 text-omega-300 px-2 py-0.5 rounded">
                {plan.steps.length} step{plan.steps.length !== 1 ? 's' : ''}
              </span>
              <span className="bg-omega-700 text-omega-300 px-2 py-0.5 rounded">
                {plan.files_affected.length} file{plan.files_affected.length !== 1 ? 's' : ''}
              </span>
            </div>
          </div>

          <div className="flex gap-2">
            <button
              onClick={handleApprove}
              className="bg-green hover:bg-green/80 text-white text-xs px-4 py-2 rounded-lg transition-colors"
            >
              Approve Plan & Start Build
            </button>
            <button
              onClick={handleGenerate}
              className="bg-omega-700 hover:bg-omega-600 text-omega-200 text-xs px-4 py-2 rounded-lg transition-colors"
            >
              Regenerate
            </button>
          </div>

          <div className="space-y-2">
            <h3 className="text-xs font-medium text-omega-400">Steps</h3>
            {plan.steps.map((step) => (
              <StepCard key={step.id} step={step} />
            ))}
          </div>

          <div className="space-y-1.5">
            <h3 className="text-xs font-medium text-omega-400">Files Affected</h3>
            {plan.files_affected.map((file, i) => (
              <div key={i} className="text-xs font-mono text-omega-500 bg-omega-800 rounded px-3 py-1.5">
                {file}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
