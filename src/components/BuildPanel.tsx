import { useEffect, useState } from 'react'
import { useChatStore } from '../stores/chatStore'
import {
  executeBuild,
  respondPermission,
  getBuildSession,
  setBuildConfig,
} from '../lib/tauri'
import type { BuildSessionEntry } from '../lib/tauri'

const actionColors: Record<string, string> = {
  create: 'text-green',
  modify: 'text-yellow',
  delete: 'text-red',
  refactor: 'text-blue',
  test: 'text-purple',
}

export function BuildPanel() {
  const {
    buildSession,
    setBuildSession,
    permissionRequest,
    setPermissionRequest,
    buildProgress,
    buildInProgress,
    setBuildInProgress,
    buildAutoApprove,
    setBuildAutoApprove,
    pipelineStatus,
    setupBuildListeners,
  } = useChatStore()

  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const unlisteners: { (): void }[] = []
    setupBuildListeners().then((unfns) => {
      unlisteners.push(...unfns)
    }).catch(console.error)

    // Load existing session
    getBuildSession().then(setBuildSession).catch(() => {})

    return () => {
      unlisteners.forEach((fn) => fn())
    }
  }, [])

  async function handleStartBuild() {
    setError(null)
    setBuildInProgress(true)
    try {
      const session = await executeBuild()
      setBuildSession(session)
    } catch (err) {
      setError(String(err))
    } finally {
      setBuildInProgress(false)
    }
  }

  async function handlePermission(approved: boolean) {
    if (!permissionRequest) return
    try {
      await respondPermission(permissionRequest.id, approved)
      setPermissionRequest(null)
    } catch (err) {
      setError(String(err))
    }
  }

  async function handleAutoApproveToggle() {
    const newVal = !buildAutoApprove
    try {
      await setBuildConfig(newVal)
      setBuildAutoApprove(newVal)
    } catch (err) {
      setError(String(err))
    }
  }

  return (
    <div className="flex-1 flex flex-col min-h-0 p-4 gap-4">
      <div className="flex items-center justify-between shrink-0">
        <h2 className="text-sm font-semibold text-omega-200">Build Pipeline</h2>
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-1.5 text-xs text-omega-400 cursor-pointer">
            <input
              type="checkbox"
              checked={buildAutoApprove}
              onChange={handleAutoApproveToggle}
              className="accent-omega-300"
            />
            Auto-approve
          </label>
          <span className={`text-xs px-2 py-0.5 rounded-full ${
            pipelineStatus === 'Building'
              ? 'bg-yellow/20 text-yellow'
              : pipelineStatus === 'Completed'
              ? 'bg-green/20 text-green'
              : pipelineStatus === 'Failed'
              ? 'bg-red/20 text-red'
              : 'bg-omega-700 text-omega-400'
          }`}>
            {pipelineStatus}
          </span>
        </div>
      </div>

      {/* Permission Request */}
      {permissionRequest && (
        <div className="bg-omega-800 border border-omega-600 rounded p-3 shrink-0">
          <div className="text-xs font-medium text-omega-200 mb-2">Permission Required</div>
          <div className="text-xs text-omega-300 mb-1">
            <span className="text-omega-400">Step #{permissionRequest.step_id}:</span>{' '}
            {permissionRequest.step_description}
          </div>
          <div className="text-xs text-omega-500 mb-3">{permissionRequest.reason}</div>
          <div className="flex gap-2">
            <button
              onClick={() => handlePermission(true)}
              className="px-3 py-1 text-xs font-medium text-omega-100 bg-green/30 hover:bg-green/40 rounded transition-colors"
            >
              Approve
            </button>
            <button
              onClick={() => handlePermission(false)}
              className="px-3 py-1 text-xs font-medium text-omega-100 bg-red/30 hover:bg-red/40 rounded transition-colors"
            >
              Deny
            </button>
          </div>
        </div>
      )}

      {/* Build Progress */}
      {buildProgress && (
        <div className="bg-omega-800 border border-omega-700 rounded p-3 shrink-0">
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs font-medium text-omega-200">Progress</span>
            <span className="text-xs text-omega-400">
              {buildProgress.completed_steps}/{buildProgress.total_steps} steps
            </span>
          </div>
          <div className="w-full h-1.5 bg-omega-700 rounded-full overflow-hidden">
            <div
              className="h-full bg-accent rounded-full transition-all duration-300"
              style={{
                width: buildProgress.total_steps > 0
                  ? `${(buildProgress.completed_steps / buildProgress.total_steps) * 100}%`
                  : '0%',
              }}
            />
          </div>
          <div className="mt-1 text-xs text-omega-500">{buildProgress.status}</div>
        </div>
      )}

      {/* Error */}
      {error && (
        <div className="bg-red/10 border border-red/30 rounded p-2 shrink-0">
          <div className="text-xs text-red">{error}</div>
        </div>
      )}

      {/* Start Build Button */}
      <div className="shrink-0">
        <button
          onClick={handleStartBuild}
          disabled={buildInProgress}
          className="w-full px-4 py-2 text-sm font-medium text-omega-100 bg-accent/80 hover:bg-accent disabled:opacity-40 rounded transition-colors"
        >
          {buildInProgress ? 'Building...' : 'Execute Build'}
        </button>
      </div>

      {/* Session Log */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        <div className="text-xs font-medium text-omega-400 mb-2">Session Log</div>
        {buildSession.length === 0 ? (
          <div className="text-xs text-omega-600 italic">No build session yet</div>
        ) : (
          <div className="space-y-1.5">
            {buildSession.map((entry, i) => (
              <SessionLogEntry key={i} entry={entry} />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

function SessionLogEntry({ entry }: { entry: BuildSessionEntry }) {
  return (
    <div className={`border-l-2 pl-3 py-1.5 ${
      entry.success ? 'border-green/40' : 'border-red/40'
    }`}>
      <div className="flex items-center gap-2">
        <span className={`text-xs font-medium ${entry.success ? 'text-green' : 'text-red'}`}>
          {entry.success ? '✓' : '✗'}
        </span>
        <span className="text-xs font-medium text-omega-200">{entry.tool}</span>
        <span className={`text-xs ${actionColors[entry.tool] ?? 'text-omega-400'}`}>
          Step #{entry.step_index + 1}
        </span>
      </div>
      <div className="text-xs text-omega-500 mt-0.5">
        {entry.duration_ms}ms
        {entry.gate_passed !== null && (
          <span className={`ml-2 ${entry.gate_passed ? 'text-green' : 'text-yellow'}`}>
            Gate: {entry.gate_passed ? 'Passed' : `Score ${entry.gate_score}`}
          </span>
        )}
      </div>
      {entry.output_preview && (
        <div className="text-xs text-omega-400 mt-0.5 truncate">{entry.output_preview}</div>
      )}
      {entry.error && (
        <div className="text-xs text-red/80 mt-0.5 truncate">{entry.error}</div>
      )}
    </div>
  )
}
