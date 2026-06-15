import { useEffect, useState } from 'react'
import {
  memoryStore,
  memorySearch,
  memoryRemember,
  memoryCount,
  memoryDelete,
  memoryClear,
} from '../lib/tauri'
import type { MemorySearchResponse, MemoryLayer } from '../lib/tauri'

export function MemoryPanel() {
  const [activeLayer, setActiveLayer] = useState<MemoryLayer>('session')
  const [key, setKey] = useState('')
  const [value, setValue] = useState('')
  const [searchQuery, setSearchQuery] = useState('')
  const [searchResults, setSearchResults] = useState<MemorySearchResponse | null>(null)
  const [rememberKey, setRememberKey] = useState('')
  const [rememberValue, setRememberValue] = useState<string | null>(null)
  const [count, setCount] = useState(0)
  const [status, setStatus] = useState<string | null>(null)

  async function refreshCount() {
    const c = await memoryCount(activeLayer)
    setCount(c)
  }

  useEffect(() => {
    refreshCount()
  }, [activeLayer])

  async function handleStore() {
    if (!key.trim() || !value.trim()) return
    try {
      const id = await memoryStore({ key: key.trim(), value: value.trim(), layer: activeLayer })
      setStatus(`Stored: ${key} → ${id.slice(0, 8)}...`)
      setKey('')
      setValue('')
      await refreshCount()
    } catch (e) {
      setStatus(`Error: ${e}`)
    }
  }

  async function handleSearch() {
    if (!searchQuery.trim()) return
    try {
      const results = await memorySearch({ query: searchQuery.trim(), layer: activeLayer, limit: 20 })
      setSearchResults(results)
      setStatus(`Found ${results.entries.length} results`)
    } catch (e) {
      setStatus(`Error: ${e}`)
    }
  }

  async function handleRemember() {
    if (!rememberKey.trim()) return
    try {
      const val = await memoryRemember(rememberKey.trim(), activeLayer)
      setRememberValue(val)
      setStatus(val ? `Remembered: ${rememberKey}` : `No value for: ${rememberKey}`)
    } catch (e) {
      setStatus(`Error: ${e}`)
    }
  }

  async function handleDelete(id: string) {
    try {
      await memoryDelete(id)
      setStatus('Deleted')
      if (searchResults) {
        setSearchResults({
          ...searchResults,
          entries: searchResults.entries.filter((e) => e.id !== id),
        })
      }
      await refreshCount()
    } catch (e) {
      setStatus(`Error: ${e}`)
    }
  }

  async function handleClear() {
    try {
      const n = await memoryClear(activeLayer)
      setStatus(`Cleared ${n} entries from ${activeLayer}`)
      setSearchResults(null)
      await refreshCount()
    } catch (e) {
      setStatus(`Error: ${e}`)
    }
  }

  const layers: MemoryLayer[] = ['session', 'project', 'user']
  const layerColors: Record<MemoryLayer, string> = {
    session: 'bg-blue-900/40 text-blue-400 border-blue-700/30',
    project: 'bg-green-900/40 text-green-400 border-green-700/30',
    user: 'bg-purple-900/40 text-purple-400 border-purple-700/30',
  }

  return (
    <div className="flex-1 overflow-y-auto p-4 space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-omega-200">Hermes Memory</h2>
        <span className="text-xs text-omega-500">{count} entries</span>
      </div>

      {/* Layer selector */}
      <div className="flex gap-2">
        {layers.map((l) => (
          <button
            key={l}
            onClick={() => setActiveLayer(l)}
            className={`flex-1 px-3 py-1.5 text-xs font-medium rounded border transition-colors ${
              activeLayer === l
                ? layerColors[l]
                : 'bg-omega-800 text-omega-400 border-omega-700 hover:bg-omega-700'
            }`}
          >
            {l.charAt(0).toUpperCase() + l.slice(1)}
          </button>
        ))}
      </div>

      {/* Status */}
      {status && (
        <div className="text-xs text-omega-400 bg-omega-800 rounded px-3 py-1.5">{status}</div>
      )}

      {/* Store */}
      <div className="bg-omega-800 border border-omega-700 rounded p-3 space-y-2">
        <div className="text-xs font-medium text-omega-400">Store</div>
        <input
          value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder="Key"
          className="w-full bg-omega-900 border border-omega-600 rounded px-2.5 py-1.5 text-xs text-omega-100 placeholder-omega-500 focus:outline-none focus:border-accent"
        />
        <textarea
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder="Value"
          rows={2}
          className="w-full bg-omega-900 border border-omega-600 rounded px-2.5 py-1.5 text-xs text-omega-100 placeholder-omega-500 focus:outline-none focus:border-accent font-mono"
        />
        <button
          onClick={handleStore}
          disabled={!key.trim() || !value.trim()}
          className="w-full bg-accent/80 hover:bg-accent disabled:opacity-40 text-white text-xs font-medium px-3 py-1.5 rounded transition-colors"
        >
          Store
        </button>
      </div>

      {/* Remember */}
      <div className="bg-omega-800 border border-omega-700 rounded p-3 space-y-2">
        <div className="text-xs font-medium text-omega-400">Remember (exact key)</div>
        <div className="flex gap-2">
          <input
            value={rememberKey}
            onChange={(e) => setRememberKey(e.target.value)}
            placeholder="Key"
            className="flex-1 bg-omega-900 border border-omega-600 rounded px-2.5 py-1.5 text-xs text-omega-100 placeholder-omega-500 focus:outline-none focus:border-accent"
          />
          <button
            onClick={handleRemember}
            disabled={!rememberKey.trim()}
            className="bg-omega-700 hover:bg-omega-600 disabled:opacity-40 text-omega-200 text-xs px-3 py-1.5 rounded transition-colors"
          >
            Get
          </button>
        </div>
        {rememberValue !== null && (
          <div className="text-xs text-omega-300 bg-omega-900 rounded px-2.5 py-1.5 font-mono">
            {rememberValue}
          </div>
        )}
      </div>

      {/* Search */}
      <div className="bg-omega-800 border border-omega-700 rounded p-3 space-y-2">
        <div className="text-xs font-medium text-omega-400">Search</div>
        <div className="flex gap-2">
          <input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="FTS5 full-text search..."
            className="flex-1 bg-omega-900 border border-omega-600 rounded px-2.5 py-1.5 text-xs text-omega-100 placeholder-omega-500 focus:outline-none focus:border-accent"
            onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
          />
          <button
            onClick={handleSearch}
            disabled={!searchQuery.trim()}
            className="bg-omega-700 hover:bg-omega-600 disabled:opacity-40 text-omega-200 text-xs px-3 py-1.5 rounded transition-colors"
          >
            Search
          </button>
        </div>

        {searchResults && searchResults.entries.length > 0 && (
          <div className="space-y-1.5 max-h-80 overflow-y-auto">
            {searchResults.entries.map((entry, i) => (
              <div
                key={entry.id}
                className="bg-omega-900 border border-omega-700 rounded px-2.5 py-2 text-xs"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className={`text-[10px] px-1 py-0.5 rounded ${
                      entry.layer === 'session' ? 'bg-blue-900/40 text-blue-400'
                      : entry.layer === 'project' ? 'bg-green-900/40 text-green-400'
                      : 'bg-purple-900/40 text-purple-400'
                    }`}>
                      {entry.layer}
                    </span>
                    <span className="font-medium text-omega-200">{entry.key}</span>
                    {searchResults.relevance[i] > 0 && (
                      <span className="text-omega-500">
                        {(searchResults.relevance[i] * 100).toFixed(0)}%
                      </span>
                    )}
                  </div>
                  <button
                    onClick={() => handleDelete(entry.id)}
                    className="text-omega-600 hover:text-red text-[10px]"
                  >
                    ✕
                  </button>
                </div>
                <div className="text-omega-400 mt-1 font-mono truncate">{entry.value}</div>
                <div className="text-omega-600 mt-0.5">{entry.timestamp}</div>
              </div>
            ))}
          </div>
        )}
        {searchResults && searchResults.entries.length === 0 && (
          <div className="text-xs text-omega-500">No results</div>
        )}
      </div>

      {/* Clear */}
      <button
        onClick={handleClear}
        className="w-full bg-omega-800 hover:bg-red/10 border border-omega-700 hover:border-red/30 text-omega-400 hover:text-red text-xs px-3 py-1.5 rounded transition-colors"
      >
        Clear {activeLayer} memory
      </button>
    </div>
  )
}
