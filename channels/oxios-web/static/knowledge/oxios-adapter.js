// Oxios Knowledge API Adapter
// Bridges files.md's file operations to Oxios REST API endpoints.
// Replaces the files.md sync protocol with simple REST CRUD.

const OXIOS_API_BASE = '/api/knowledge';

// ---------------------------------------------------------------------------
// Knowledge Directory — files live in ~/.oxios/workspace/knowledge/
// ---------------------------------------------------------------------------

// Override: use Oxios REST API for file operations instead of sync protocol.
// The adapter provides functions compatible with files.md's file system API
// but routes through Oxios backend.

/**
 * Read a knowledge file from Oxios API.
 * @param {string} path - relative path within knowledge/ (e.g. "brain/Rust.md")
 * @returns {Promise<string|null>} file content or null
 */
async function oxiosReadFile(path) {
    try {
        const response = await fetch(`${OXIOS_API_BASE}/file/${encodeURIComponent(path)}`, {
            method: 'GET',
            credentials: 'include',
            headers: { 'Accept': 'text/plain' }
        });
        if (response.status === 404) return null;
        if (!response.ok) {
            console.error('oxiosReadFile error:', response.status, await response.text());
            return null;
        }
        return await response.text();
    } catch (e) {
        console.error('oxiosReadFile network error:', e);
        return null;
    }
}

/**
 * Write a knowledge file via Oxios API.
 * @param {string} path - relative path within knowledge/
 * @param {string} content - file content
 * @returns {Promise<boolean>} success
 */
async function oxiosWriteFile(path, content) {
    try {
        const response = await fetch(`${OXIOS_API_BASE}/file/${encodeURIComponent(path)}`, {
            method: 'PUT',
            credentials: 'include',
            headers: { 'Content-Type': 'text/plain' },
            body: content
        });
        return response.ok;
    } catch (e) {
        console.error('oxiosWriteFile error:', e);
        return false;
    }
}

/**
 * Delete a knowledge file via Oxios API.
 * @param {string} path - relative path within knowledge/
 * @returns {Promise<boolean>} success
 */
async function oxiosDeleteFile(path) {
    try {
        const response = await fetch(`${OXIOS_API_BASE}/file/${encodeURIComponent(path)}`, {
            method: 'DELETE',
            credentials: 'include'
        });
        return response.ok;
    } catch (e) {
        console.error('oxiosDeleteFile error:', e);
        return false;
    }
}

/**
 * Get file tree from Oxios API.
 * @param {string} [dir] - optional subdirectory
 * @returns {Promise<Array>} tree entries [{name, is_dir, size}]
 */
async function oxiosGetTree(dir) {
    try {
        let url = `${OXIOS_API_BASE}/tree`;
        if (dir) url += `?dir=${encodeURIComponent(dir)}`;
        const response = await fetch(url, {
            credentials: 'include',
            headers: { 'Accept': 'application/json' }
        });
        if (!response.ok) return [];
        return await response.json();
    } catch (e) {
        console.error('oxiosGetTree error:', e);
        return [];
    }
}

/**
 * Search knowledge files via Oxios API.
 * @param {string} query - search query
 * @param {number} [limit=20] - max results
 * @returns {Promise<Array>} search results
 */
async function oxiosSearch(query, limit) {
    try {
        const response = await fetch(`${OXIOS_API_BASE}/search`, {
            method: 'POST',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ query, limit: limit || 20 })
        });
        if (!response.ok) return [];
        const data = await response.json();
        return data.results || [];
    } catch (e) {
        console.error('oxiosSearch error:', e);
        return [];
    }
}

/**
 * Get backlinks for a file via Oxios API.
 * @param {string} path - file path
 * @returns {Promise<Array>} backlinks
 */
async function oxiosGetBacklinks(path) {
    try {
        const response = await fetch(`${OXIOS_API_BASE}/backlinks?path=${encodeURIComponent(path)}`, {
            credentials: 'include',
            headers: { 'Accept': 'application/json' }
        });
        if (!response.ok) return [];
        return await response.json();
    } catch (e) {
        console.error('oxiosGetBacklinks error:', e);
        return [];
    }
}

/**
 * Get link graph via Oxios API.
 * @returns {Promise<Object>} graph data {nodes, edges}
 */
async function oxiosGetGraph() {
    try {
        const response = await fetch(`${OXIOS_API_BASE}/graph`, {
            credentials: 'include',
            headers: { 'Accept': 'application/json' }
        });
        if (!response.ok) return { nodes: [], edges: [] };
        return await response.json();
    } catch (e) {
        console.error('oxiosGetGraph error:', e);
        return { nodes: [], edges: [] };
    }
}

/**
 * Copilot chat via Oxios API.
 * @param {string} question - user question
 * @param {string} [contextPath] - current file being edited
 * @returns {Promise<Object>} copilot response {content, referenced_notes}
 */
async function oxiosCopilotChat(question, contextPath) {
    try {
        const response = await fetch(`${OXIOS_API_BASE}/copilot`, {
            method: 'POST',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ question, context_path: contextPath || null })
        });
        if (!response.ok) {
            return { content: 'Error: ' + response.status, referenced_notes: [] };
        }
        return await response.json();
    } catch (e) {
        console.error('oxiosCopilotChat error:', e);
        return { content: 'Network error: ' + e.message, referenced_notes: [] };
    }
}

// ---------------------------------------------------------------------------
// Oxios Mode Flag
// ---------------------------------------------------------------------------

// When running inside Oxios, we bypass the files.md sync protocol
// and use simple REST CRUD instead.
const OXIOS_MODE = true;

// Override API_URL to prevent connecting to files.md server
// (keeping compatibility with post() function in app.js)
const OXIOS_API_URL = '';

// Mark server as "OK" immediately since we don't need a separate auth flow
function oxiosMarkReady() {
    localStorage.setItem('lastServerOk', Date.now().toString());
}

// Auto-initialize on load
if (OXIOS_MODE) {
    oxiosMarkReady();
}
