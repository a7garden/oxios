// Forbidden chars on Windows / PWA / Unix.
const FORBIDDEN_FILENAME_CHARS = ['<', '>', ':', '"', '|', '\\', '?', '*', '\x00', '/'];

// Oxios: REST API base path for knowledge operations.
const OXIOS_KNOWLEDGE_API = '/api/knowledge';

// Oxios: REST API implementations for knowledge file operations.
// These replace the File System Access API calls when running inside Oxios.

async function oxiosRead(path) {
    const relPath = path.replace(/^\/+/, ''); // "/brain/Rust.md" -> "brain/Rust.md"
    const resp = await fetch(`${OXIOS_KNOWLEDGE_API}/file/${encodeURIComponent(relPath)}`, {
        method: 'GET',
        credentials: 'include',
        headers: { 'Accept': 'text/plain' }
    });
    if (resp.status === 404) throw new Error('File not found: ' + relPath);
    if (!resp.ok) throw new Error('Read error: ' + resp.status + ' ' + resp.statusText);
    return await resp.text();
}

async function oxiosWrite(path, content) {
    const relPath = path.replace(/^\/+/, '');
    const resp = await fetch(`${OXIOS_KNOWLEDGE_API}/file/${encodeURIComponent(relPath)}`, {
        method: 'PUT',
        credentials: 'include',
        headers: { 'Content-Type': 'text/plain' },
        body: content
    });
    if (!resp.ok) {
        const errText = await resp.text().catch(() => '');
        throw new Error('Write error: ' + resp.status + ' ' + resp.statusText + ' ' + errText);
    }
}

async function oxiosRemove(path) {
    const relPath = path.replace(/^\/+/, '');
    const resp = await fetch(`${OXIOS_KNOWLEDGE_API}/file/${encodeURIComponent(relPath)}`, {
        method: 'DELETE',
        credentials: 'include'
    });
    if (!resp.ok && resp.status !== 404) {
        const errText = await resp.text().catch(() => '');
        throw new Error('Delete error: ' + resp.status + ' ' + errText);
    }
}

async function oxiosExists(path) {
    const relPath = path.replace(/^\/+/, '');
    try {
        const resp = await fetch(`${OXIOS_KNOWLEDGE_API}/file/${encodeURIComponent(relPath)}`, {
            method: 'HEAD',
            credentials: 'include'
        });
        return resp.ok;
    } catch {
        return false;
    }
}

async function oxiosGetTree(dir = '') {
    let url = `${OXIOS_KNOWLEDGE_API}/tree`;
    if (dir) url += `?dir=${encodeURIComponent(dir)}`;
    try {
        const resp = await fetch(url, {
            credentials: 'include',
            headers: { 'Accept': 'application/json' }
        });
        if (!resp.ok) return [];
        return await resp.json();
    } catch {
        return [];
    }
}

async function oxiosWriteAtEnd(path, content) {
    const relPath = path.replace(/^\/+/, '');
    let existing = '';
    try {
        const resp = await fetch(`${OXIOS_KNOWLEDGE_API}/file/${encodeURIComponent(relPath)}`, {
            method: 'GET',
            credentials: 'include',
            headers: { 'Accept': 'text/plain' }
        });
        if (resp.ok) existing = await resp.text();
    } catch {
        // File doesn't exist yet, start empty
    }
    const updated = existing + content;
    await oxiosWrite(relPath, updated);
}

// Oxios: Flag that is set from app.js. When true, all file operations
// go through the REST API instead of File System Access API.
let oxiosMode = false;

// Oxios: Enable Oxios REST API mode. Call this from app.js init().
function enableOxiosMode() {
    oxiosMode = true;
    log('Oxios mode: REST API file operations enabled');
}

function sanitizeFilename(filename) {
    return FORBIDDEN_FILENAME_CHARS.reduce((result, ch) => result.replaceAll(ch, ''), filename);
}

function findForbiddenChar(name) {
    for (const ch of FORBIDDEN_FILENAME_CHARS) {
        if (name.includes(ch)) return ch;
    }
    return null;
}

async function getFileHandle(path, create = false) {
    if (oxiosMode) {
        // In Oxios mode, we don't use File System Access handles.
        // The REST API is used instead. Return a mock object for compatibility.
        return { oxiosPath: path };
    }

    let dir, filename;
    if (path.includes('/')) {
        const parts = path.split('/');
        filename = parts.pop();
        dir = parts.join('/');
    } else {
        dir = '';
        filename = path;
    }

    const dirs = dir.split('/');
    let currentDirHandle = await getRootDirHandle();
    for (const dirName of dirs) {
        if (dirName) {
            try {
                currentDirHandle = await currentDirHandle.getDirectoryHandle(dirName, {create: create});
            } catch (error) {
                throw error;
            }
        }
    }

    let fileHandle;
    try {
        fileHandle = await currentDirHandle.getFileHandle(filename, {create: create});
    } catch (error) {
        throw error;
    }

    return fileHandle;
}

async function read(path) {
    if (oxiosMode) return oxiosRead(path);
    let fileHandle = await getFileHandle(path);
    let file = await fileHandle.getFile();
    return await file.text();
}

async function write(path, content) {
    if (oxiosMode) return oxiosWrite(path, content);
    let fileHandle = await getFileHandle(path, true);
    const writable = await fileHandle.createWritable();
    await writable.write(content);
    await writable.close();
}

async function writeAtEnd(path, content) {
    if (oxiosMode) return oxiosWriteAtEnd(path, content);
    let fileHandle = await getFileHandle(path, true);
    if (fileHandle === null) {
        // TODO fix once Chromium fixes the bug
        throw new Error('Invalid file name');
    }

    const writable = await fileHandle.createWritable({ keepExistingData: true });
    await writable.seek(await fileHandle.getFile().then(file => file.size));
    await writable.write(content);
    await writable.close();

    const file = await fileHandle.getFile();
    return file.lastModified;
}

// TODO save metadata & files
// Write only if content is different.
async function writeIfContentIsDifferent(path, content) {
    if (oxiosMode) {
        // Oxios: always write through REST API
        await oxiosWrite(path, content);
        return Date.now();
    }
    let fileHandle = await getFileHandle(path, true);
    if (fileHandle === null) {
        // TODO fix once Chromium fixes the bug
        throw new Error('Invalid file name');
    }

    const fileExists = !await exists(path);
    if (fileExists || !await isContentEqual(path, content)) {
        // TODO what if we're syncing first time and already have changes?
        log('Hashes do not match, writing file...', path);
        const writable = await fileHandle.createWritable();
        await writable.write(content);
        await writable.close();
    } else {
        log('Hashes match, no need to write file.');
    }

    const file = await fileHandle.getFile();
    return file.lastModified;
}

// Works only for files.
async function exists(path) {
    if (oxiosMode) return oxiosExists(path);
    try {
        await getFileHandle(path);
        return true;
    } catch (error) {
        if (error.name === 'NotFoundError') {
            return false
        }
        throw error
    }
}

async function remove(path) {
    if (oxiosMode) return oxiosRemove(path);
    let fileHandle = await getFileHandle(path);
    if (fileHandle === null) {
        // TODO fix once Chromium fixes the bug
        logError('Malformed name, skipping file...');
        return;
    }
    await fileHandle.remove()
    log(`File ${path} removed successfully.`);

    removeMemFile(path);
}

async function rename(oldpath, newpath) {
    if (oxiosMode) {
        // Oxios: read via REST, write via REST, then delete
        const content = await oxiosRead(oldpath);
        await oxiosWrite(newpath, content);
        await oxiosRemove(oldpath);
        return;
    }
    let content = await read(oldpath)
    await write(newpath, content)
    await remove(oldpath)
}

// removeDir deletes a directory and everything under it. Files are deleted
// one-by-one so the in-memory file tree and server-sync bookkeeping stay in
// sync.
async function removeDir(dirPath) {
    const filePaths = collectFilePathsInDir(dirPath);
    for (const p of filePaths) {
        try {
            await remove(p);
        } catch (err) {
            logError('removeDir: failed to remove file', p, err);
        }
    }

    const parts = trimPrefix(dirPath, '/').split('/').filter(Boolean);
    const dirName = parts.pop();

    if (!oxiosMode) {
        const rootHandle = await getRootDirHandle();
        let parentHandle = rootHandle;
        for (const seg of parts) {
            parentHandle = await parentHandle.getDirectoryHandle(seg);
        }
        try {
            await parentHandle.removeEntry(dirName, { recursive: true });
        } catch (err) {
            logError('removeDir: removeEntry failed', dirPath, err);
        }
    }

    removeMemDir(dirPath);
    log(`Dir ${dirPath} removed.`);
}

// moveDir moves every file under oldDirPath into newDirPath, which can be in
// any parent (including a different one). Per-file moves keep server-sync
// bookkeeping intact; afterwards the empty old directory entry is removed.
async function moveDir(oldDirPath, newDirPath) {
    if (newDirPath === oldDirPath) return;
    // Disallow moving a folder into itself or any of its own descendants -
    // we'd otherwise loop forever copying the dir into a subpath of itself.
    if (newDirPath === oldDirPath + '/' || newDirPath.startsWith(oldDirPath + '/')) {
        logError('moveDir: refusing to move dir into itself', oldDirPath, newDirPath);
        return;
    }

    const filePaths = collectFilePathsInDir(oldDirPath);
    if (filePaths.length === 0) {
        await createDir(newDirPath);
    }
    let allMoved = true;
    for (const oldFilePath of filePaths) {
        const rel = oldFilePath.slice(oldDirPath.length);
        const newFilePath = newDirPath + rel;
        try {
            await moveFile(oldFilePath, newFilePath);
        } catch (err) {
            logError('moveDir: failed to move file', oldFilePath, err);
            allMoved = false;
        }
    }

    if (!allMoved) {
        // Some file didn't make it across. A recursive remove now would
        // silently take those leftovers with it, so leave the old dir alone.
        logError('moveDir: not all files moved, leaving old dir in place', oldDirPath);
        return;
    }

    if (!oxiosMode) {
        const oldParts = trimPrefix(oldDirPath, '/').split('/').filter(Boolean);
        const oldDirName = oldParts.pop();
        const rootHandle = await getRootDirHandle();
        let oldParentHandle = rootHandle;
        for (const seg of oldParts) {
            oldParentHandle = await oldParentHandle.getDirectoryHandle(seg);
        }
        try {
            await oldParentHandle.removeEntry(oldDirName, { recursive: true });
        } catch (err) {
            logError('moveDir: removeEntry old dir failed', oldDirPath, err);
        }
    }

    removeMemDir(oldDirPath);
    log(`Dir ${oldDirPath} moved to ${newDirPath}.`);
}

// renameDir moves every file under oldDirPath into a sibling directory called
// newName.
async function renameDir(oldDirPath, newName) {
    const parts = trimPrefix(oldDirPath, '/').split('/').filter(Boolean);
    parts.pop();
    const parentPath = '/' + parts.join('/');
    const newDirPath = joinPath(parentPath, newName);
    await moveDir(oldDirPath, newDirPath);
}

// collectFilePathsInDir returns absolute paths of every file under dirPath,
// using the in-memory file tree so we don't hit OPFS for the listing.
function collectFilePathsInDir(dirPath) {
    const collected = [];
    walk(files, (p, isFile) => {
        if (!isFile) return;
        if (p === dirPath || p.startsWith(dirPath + '/')) {
            collected.push(p);
        }
    });
    return collected;
}

// removeMemDir drops a directory subtree from the in-memory file map.
function removeMemDir(dirPath) {
    const parts = trimPrefix(dirPath, '/').split('/').filter(Boolean);
    const dirName = parts.pop();
    let cur = files;
    for (const seg of parts) {
        cur = cur[seg + '/'];
        if (!cur) return;
    }
    delete cur[dirName + '/'];
}

async function mkdir(path) {
    if (oxiosMode) {
        // In Oxios mode, directories are created automatically on write.
        // No explicit mkdir needed for the REST API.
        return;
    }
    try {
        let currentDirHandle = await getRootDirHandle();
        await currentDirHandle.getDirectoryHandle(path, {create: true});
    } catch (e) {
        logError(e);
        throw e;
    }
}

async function mkdirAll(path) {
    const dirs = path.split('/');
    let currentDirHandle = await getRootDirHandle();
    for (const dirName of dirs) {
        if (dirName) {
            await mkdir(path)
        }
    }
}

// createDir creates an empty directory on OPFS at the given path and registers
// it in the in-memory file tree so the sidebar picks it up.
async function createDir(dirPath) {
    if (oxiosMode) {
        // Oxios: directories are created automatically on file write.
        // Still register in the in-memory file tree.
        let cur = files;
        const parts = trimPrefix(dirPath, '/').split('/').filter(Boolean);
        for (const seg of parts) {
            const key = seg + '/';
            if (!cur[key]) cur[key] = {};
            cur = cur[key];
        }
        log(`Dir ${dirPath} created (in-memory).`);
        return;
    }
    const parts = trimPrefix(dirPath, '/').split('/').filter(Boolean);
    if (parts.length === 0) return;

    let dirHandle = await getRootDirHandle();
    for (const seg of parts) {
        dirHandle = await dirHandle.getDirectoryHandle(seg, { create: true });
    }

    let cur = files;
    for (const seg of parts) {
        const key = seg + '/';
        if (!cur[key]) cur[key] = {};
        cur = cur[key];
    }
    log(`Dir ${dirPath} created.`);
}

async function writeMediaFile(fileName, file) {
    if (oxiosMode) {
        // Phase 1: media upload not implemented for Oxios REST API.
        log('Oxios: media upload via REST API not implemented yet');
        return null;
    }
    try {
        const rootHandle = await getRootDirHandle();

        let mediaHandle;
        try {
            mediaHandle = await rootHandle.getDirectoryHandle('media');
        } catch {
            mediaHandle = await rootHandle.getDirectoryHandle('media', {create: true});
        }

        const fileHandle = await mediaHandle.getFileHandle(fileName, {create: true});
        const writable = await fileHandle.createWritable();
        await writable.write(file);
        await writable.close();

        const path = '/media/' + fileName;
        addMemFile(path, {
            isFile: true,
            path: path,
            imageUrl: await getImageUrl(fileHandle),
        });

        return fileHandle;
    } catch (error) {
        logError('Error saving file:', error);
        return null;
    }
}

function generateSafeFilename(originalName) {
    const now = new Date();
    const timestamp = `${String(now.getDate()).padStart(2, '0')}.${String(now.getMonth() + 1).padStart(2, '0')}.${now.getFullYear()} ${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}`;
    return `${timestamp}-${originalName}`.replace(/[<>:"/\\|?*\s]/g, '-');
}