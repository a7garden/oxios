// When there's no opened local dir, a temporary FS is provided.
// Temporary FS includes welcome files, so to demonstrate the app.
// First we try to create OPFS storage, fallback to our own in-memory FS on failure.

async function getTemporaryStorageDirHandle() {
    // Safari ships OPFS but its FileSystemFileHandle exposes only
    // createSyncAccessHandle (worker-only). Older Chromium has no
    // FileSystemFileHandle.remove(). Fall back to the in-memory FS if
    // either of those write/delete entry points is missing so app code
    // doesn't blow up mid-flow.
    if (!opfsIsFullyUsable()) {
        console.warn('OPFS missing createWritable or remove, using in-memory FS');
        isMemFS = true;
        return getMemFSRoot();
    }

    // OPFS requires a secure context (https or localhost), not available on file://
    try {
        const root = await navigator.storage.getDirectory();

        // Skip the seed only when the LAST welcome file is already on disk.
        // Using "any entries" as the seeded marker is racy: a reader
        // arriving mid-seed sees the first dir already written and returns
        // root with partial contents. Probing for a file that's written
        // last means concurrent callers all re-run the seed (idempotent
        // via {create:true}) and none of them returns until all welcome
        // files exist.
        const lastFile = Object.keys(WELCOME_FILES).pop();
        let seeded = true;
        try { await root.getFileHandle(lastFile); }
        catch { seeded = false; }

        if (seeded) {
            return root;
        }

        // If a welcome file was archived (moved to /archive/), don't
        // re-seed it. Archive flattens names, so a Set of names covers
        // both root-level and nested welcome files.
        const archived = new Set();
        try {
            const archiveDir = await root.getDirectoryHandle('archive');
            for await (const entry of archiveDir.values()) {
                if (entry.kind === 'file') archived.add(entry.name);
            }
        } catch { /* no archive dir yet */ }

        async function createFiles(obj, dirHandle) {
            for (const [name, data] of Object.entries(obj)) {
                if (data.isFile) {
                    if (archived.has(name)) continue;
                    const fileHandle = await dirHandle.getFileHandle(name, { create: true });
                    const writable = await fileHandle.createWritable();
                    await writable.write(data.content);
                    await writable.close();
                } else {
                    const subDirHandle = await dirHandle.getDirectoryHandle(removeTrailingSlash(name), { create: true });
                    await createFiles(data, subDirHandle);
                }
            }
        }
        await createFiles(WELCOME_FILES, root);

        return root;
    } catch (e) {
        console.warn('OPFS unavailable, using in-memory FS:', e.message);
        isMemFS = true;
        return getMemFSRoot();
    }
}

// Returns true only when the browser exposes every FileSystemFileHandle
// method the app relies on. Used by both the temporary-storage path and
// app.js's getRootDirHandle to decide between OPFS and the in-memory FS.
function opfsIsFullyUsable() {
    if (typeof FileSystemFileHandle === 'undefined') return false;
    const proto = FileSystemFileHandle.prototype;
    return typeof proto.createWritable === 'function'
        && typeof proto.remove === 'function';
}

let memFSRoot = null;
function getMemFSRoot() {
    if (memFSRoot) return memFSRoot;

    memFSRoot = new MemDir('');
    function populate(obj, parent) {
        for (const [name, data] of Object.entries(obj)) {
            if (data.isFile) {
                const file = new MemFile(name, data.content || '');
                file.parent = parent;
                parent.entries[name] = file;
            } else {
                const dir = new MemDir(removeTrailingSlash(name));
                parent.entries[dir.name] = dir;
                populate(data, dir);
            }
        }
    }
    populate(WELCOME_FILES, memFSRoot);

    return memFSRoot;
}

class MemFile {
    constructor(name, content = '') {
        this.kind = 'file';
        this.name = name;
        this.content = content;
        this.lastModified = Date.now();
        this.parent = null;
    }

    async getFile() {
        const content = this.content;
        return {
            name: this.name,
            lastModified: this.lastModified,
            size: new Blob([content]).size,
            type: '',
            text: async () => content,
            arrayBuffer: async () => new TextEncoder().encode(content).buffer,
        };
    }

    async createWritable(opts = {}) {
        let buffer = opts.keepExistingData ? this.content : '';
        let pos = opts.keepExistingData ? buffer.length : 0;
        const self = this;
        return {
            async write(data) {
                const text = typeof data === 'string' ? data : await new Blob([data]).text();
                buffer = buffer.slice(0, pos) + text + buffer.slice(pos + text.length);
                pos += text.length;
            },
            async seek(offset) { pos = offset; },
            async close() {
                self.content = buffer;
                self.lastModified = Date.now();
            },
        };
    }

    async remove() {
        if (this.parent) delete this.parent.entries[this.name];
    }
}

class MemDir {
    constructor(name) {
        this.kind = 'directory';
        this.name = name;
        this.entries = {};
    }

    async getDirectoryHandle(name, opts = {}) {
        if (!this.entries[name]) {
            if (!opts.create) throw new DOMException(`"${name}" not found`, 'NotFoundError');
            this.entries[name] = new MemDir(name);
        }
        return this.entries[name];
    }

    async getFileHandle(name, opts = {}) {
        if (!this.entries[name]) {
            if (!opts.create) throw new DOMException(`"${name}" not found`, 'NotFoundError');
            const file = new MemFile(name);
            file.parent = this;
            this.entries[name] = file;
        }
        return this.entries[name];
    }

    // Mirrors FileSystemDirectoryHandle.removeEntry. Required by fs.js's
    // remove(path) and removeDir(dirPath); the non-standard
    // fileHandle.remove() isn't available on Safari OPFS or this in-memory
    // FS, so fs.js prefers the parent-directory form.
    async removeEntry(name, opts = {}) {
        const entry = this.entries[name];
        if (!entry) {
            throw new DOMException(`"${name}" not found`, 'NotFoundError');
        }
        if (entry.kind === 'directory' && Object.keys(entry.entries).length > 0 && !opts.recursive) {
            throw new DOMException('Directory not empty', 'InvalidModificationError');
        }
        delete this.entries[name];
    }

    async *values() {
        for (const entry of Object.values(this.entries)) yield entry;
    }
}

const WELCOME_FILES = {
    "brain/": {
        "We think that we understand, but in reality we just know.md": {
            "content": "Reading and rereading can easily fool us into believing that we understand a text. Rereading is especially dangerous because of the mere-exposure effect: The moment we become familiar with something, we start believing we also understand it. On top of that, we also tend to like it it more.\n\n[Brain is the most complex object in known universe](/brain/Brain%20is%20the%20most%20complex%20object%20in%20known%20universe.md)",
            isFile: true,
        },
        "Brain is the most complex object in known universe.md": {
            "content": "Nothing will make you appreciate human intelligence like learning about how unbelievably challenging it is to try to create a computer as smart as we are. Building skyscrapers, putting humans in space, figuring out the details of how the Big Bang went down - all far easier than understanding our own brain or how to make something as cool as it\n\n[We think that we understand, but in reality we just know](/brain/We%20think%20that%20we%20understand,%20but%20in%20reality%20we%20just%20know.md)",
            isFile: true,
        },
        "Change your environment instead of using willpower.md": {
            "content": "When scientists analyze people who appear to have tremendous self-control, it turns out those individuals aren’t all that different from those who are struggling. Instead, “disciplined” people are better at structuring their lives in a way that does not require heroic willpower and self-control.\n",
            isFile: true,
        },
    },
    "happiness/": {
        "Meditation.md": {
            "content": "Once you are relaxed, picture yourself living in an abundant world. In this abundant world, there are no restraints or limitations. Good things flow past you continuously. Imagine every abundant thing you have ever desired – car, home, friends, love, joy, wealth, success, peace of mind, challenge. Visualize yourself living your life surrounded by this abundance. Repeat this visualization several times a day until it begins to feel real to you. Open your arms, your heart, and your mind. Get out of the way, and let it happen.\n\n[Boredom is just an emotion](/happiness/Boredom%20is%20just%20an%20emotion.md)",
            isFile: true,
        },
        "Boredom is just an emotion.md": {
            "content": "It's not an indicator that you're doing something wrong in your life\n\nBefore we had phones and technologies we would just sit around the fire and we would talk and we wouldn't call that boring that was just life\n\nAnd bow we have that endless need for entertainment, anything when nothing is happening we think it's wrong and we need to fix it\n\nNon eventfulness is just a part of our life and you can embrace it as\npeace or you can frantically try to create more chaos\n\n[Meditation](/happiness/Meditation.md)",
            isFile: true,
        },
    },
    "🪴 Welcome.md": {
        "content":
            "To store files in a local folder, [open or create folder](cmd:openDir).\n\n" +
            "Use [chat](cmd:openChat) to dump whatever is on your mind.\n\n" +
            // "Press `Cmd+K` or `Ctrl+K` to quick switch between notes.\n\n" +
            "[Markdown Guide](/Markdown%20Guide.md)\n[Hotkeys](/Hotkeys.md)\n[Links](/Links.md)",
        isFile: true,
    },
    "Links.md": {
        "content": "Links are important.\n" +
            "\n" +
            "Relations among ideas are far more important than the ideas themselves.\n" +
            "Learning is making meaningful connections.\n\n" +
            "Type `[` to insert a new link.\n\n" +
            "[Markdown Guide](/Markdown%20Guide.md)",
        isFile: true,
    },
    "Markdown Guide.md": {
        "content":
            "Use `#` for headers. More `#` symbols create smaller headers.\n" +
            "\n" +
            "#### Text Formatting\n" +
            "- **Bold text** using `**bold**` or `__bold__` **(Cmd/Ctrl + B)**\n" +
            "- *Italic text* using `*italic*` or `_italic_` **(Cmd/Ctrl + I)**\n" +
            "- ***Bold and italic*** using `***text***`\n" +
            "- ~~Strikethrough~~ using `~~text~~`\n" +
            "- `Inline code` using backticks\n" +
            "\n" +
            "#### Lists\n" +
            "- First item\n" +
            "- Second item\n" +
            "  - Third item\n\n" +
            "1. First item\n" +
            "2. Second item\n" +
            "   1. Third item\n" +
            "\n" +
            "#### Checklist\n" +
            "- [x] Completed task\n" +
            "- [ ] Incomplete task\n" +
            "Format:\n`- [ ] Item`\n" +
            "\n" +
            "#### Blockquotes\n" +
            ">This is a blockquote. It can span multiple lines and is great for highlighting important information or quotes from other sources.\n" +
            "\nFormat:\n`> This is a blockquote`\n" +
            "\n" +
            "#### Code Blocks\n" +
            "```\n" +
            "Here is some code.\n" +
            "```\n" +
            "\n" +
            "#### Images\n" +
            "![Why taking notes](https://app.files.md/img/notes.jpg)\n" +
            "\n" +
            "*You can paste your own images via `Cmd/Ctrl + V`*\n\n" +
            "#### Links\n" +
            "You can insert your own links by typing `[`.\n\n" +
            "[Links](/Links.md)\n" +
            "[My project](/My%20project.md)",
        isFile: true,
    },
    "Hotkeys.md": {
        "content":
            "| Hotkey | Action |\n" +
            "| -------- | -------- |\n" +
            "| `[` | Insert a link to a file |\n" +
            "| `Cmd+P` / `Ctrl+P` | Open file search modal |\n" +
            "| `Cmd+N` / `Ctrl+N` | New file |\n" +
            "| `Cmd+M` / `Ctrl+M` | Move file |\n" +
            "| `Cmd+D` / `Ctrl+D` | Delete file |\n" +
            "| `Cmd+Enter` / `Ctrl+Enter` | Open chat |\n" +
            "| `Cmd+Shift+Enter` / `Ctrl+Shift+Enter` | Toggle chat dialog |\n" +
            "| `Cmd+[` / `Ctrl+[`  | Go to previous file   |\n" +
            "| `Cmd+]` / `Ctrl+]`  | Go to next file  |\n" +
            "| `Cmd+~` / `Ctrl+~` | Toggle sidebar |\n" +
            "| `Cmd+B` / `Ctrl+B` | Toggle **bold** formatting |\n" +
            "| `Cmd+I` / `Ctrl+I` | Toggle *italic* formatting |\n" +
            "| `Cmd` / `Ctrl` + `Click` | Copy from `code` element |\n" +
            "| `Cmd` / `Ctrl` + `Click` | Open a link  |\n" +
            "| `Ctrl` + `Cmd` + `Space` | Insert emoji (MacOS) |\n" +
            "\n" +
            "[Markdown Guide](/Markdown%20Guide.md)",
        isFile: true,
    },
    "My project.md": {
        "content": "You can dump project related thoughts here.",
        isFile: true,
    },
}

function getHelpContent() {
    // Concatenate Hotkeys and Markdown Guide into one Help.md. Drop any
    // line that is solely a `[text](file.md)` link - those references
    // are dead-ends once the welcome files are merged into one
    // document, and the user wants them gone (not just the syntax).
    // Anchored to line start so a stray "[" inside an inline-code
    // table cell (e.g. ``| `[` |`` in the Hotkeys table) can't be
    // mistaken for a link. Then collapse the blank gaps the removal
    // leaves behind so we don't end up with three+ newlines in a row.
    const stripMdFileLinks = s => s
        .replace(/^[ \t]*\[[^\]\n]+\]\([^)\n]*\.md\)[ \t]*\n?/gm, '')
        .replace(/\n{3,}/g, '\n\n');
    // Drop the "#### Images" chapter - it points at an external image
    // and isn't useful in the merged Help.md. Match runs until the
    // next #### header or end of string.
    const stripImagesChapter = s => s.replace(/#### Images\n[\s\S]*?(?=#### |$)/g, '');
    return stripImagesChapter(stripMdFileLinks(
        "### Hotkeys\n\n" +
        WELCOME_FILES["Hotkeys.md"].content +
        "\n\n" +
        "### Markdown Guide\n\n" +
        WELCOME_FILES["Markdown Guide.md"].content
    ));
}

