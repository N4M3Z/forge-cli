// Shared tuicr-style code annotation. Operates on any `pre code[data-path]`
// block: wraps each line with a number gutter, lets you attach typed notes,
// and exports them as a tuicr review comment. Keyed by `path:line` so multiple
// blocks on one page (e.g. per-harness settings files) never collide.
(function () {
    var annotations = {};
    var TYPES = [
        { id: 'issue', label: 'ISSUE', color: 'var(--red)', def: 'problems to fix' },
        { id: 'note', label: 'NOTE', color: 'var(--accent)', def: 'observations' },
        { id: 'suggestion', label: 'SUGGESTION', color: 'var(--amber)', def: 'improvements' },
        { id: 'praise', label: 'PRAISE', color: 'var(--green)', def: 'positive feedback' }
    ];

    function languageFromClass(codeEl) {
        var match = (codeEl.className || '').match(/language-([\w-]+)/);
        return match ? match[1] : '';
    }

    function highlightLine(line, language) {
        var hasHljs = typeof hljs !== 'undefined' && hljs.getLanguage;
        if (hasHljs && language && hljs.getLanguage(language)) {
            return hljs.highlight(line, { language: language }).value;
        }
        return line.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    function initBlock(codeEl) {
        if (codeEl.dataset.annoInit) return;
        codeEl.dataset.annoInit = 'true';
        codeEl.classList.add('hljs');
        var path = codeEl.dataset.path || '';
        var baseLanguage = languageFromClass(codeEl);
        var isMarkdown = baseLanguage === 'markdown' || baseLanguage === 'md';
        var lines = codeEl.textContent.split('\n');
        var inFrontmatter = isMarkdown && lines.length > 0 && lines[0].trim() === '---';
        var frontmatterClosed = false;
        codeEl.innerHTML = lines.map(function (line, index) {
            var number = index + 1;
            var language = baseLanguage;
            if (isMarkdown) {
                if (inFrontmatter && !frontmatterClosed) {
                    language = 'yaml';
                    if (index > 0 && line.trim() === '---') { frontmatterClosed = true; }
                } else {
                    language = 'markdown';
                }
            }
            return '<span class="code-line" data-line="' + number + '">' +
                '<span class="line-num">' + number + '</span>' +
                '<span class="line-content">' + highlightLine(line, language) + '</span></span>';
        }).join('');
        codeEl.addEventListener('click', function (event) {
            if (event.target.closest('.annotation')) return;
            var lineEl = event.target.closest('.code-line');
            if (!lineEl || lineEl.querySelector('.type-picker')) return;
            showTypePicker(lineEl, path, lineEl.dataset.line);
        });
    }

    function showTypePicker(lineEl, path, lineNumber) {
        document.querySelectorAll('.type-picker').forEach(function (picker) { picker.remove(); });
        var picker = document.createElement('div');
        picker.className = 'type-picker';
        var label = document.createElement('span');
        label.className = 'picker-label';
        label.textContent = 'Add note on line ' + lineNumber;
        picker.appendChild(label);
        TYPES.forEach(function (type) {
            var button = document.createElement('button');
            button.className = 'picker-type';
            button.textContent = type.label;
            button.style.color = type.color;
            button.onclick = function (event) {
                event.stopPropagation();
                picker.remove();
                addAnnotation(lineEl, path, lineNumber, type);
            };
            picker.appendChild(button);
        });
        var cancel = document.createElement('button');
        cancel.className = 'picker-cancel';
        cancel.textContent = 'Cancel';
        cancel.onclick = function (event) { event.stopPropagation(); picker.remove(); };
        picker.appendChild(cancel);
        lineEl.after(picker);
    }

    function addAnnotation(lineEl, path, lineNumber, type) {
        var key = path + ':' + lineNumber;
        var annotation = document.createElement('div');
        annotation.className = 'annotation';
        annotation.style.borderLeftColor = type.color;
        annotation.innerHTML = '<span class="annot-badge" style="color:' + type.color + '">' + type.label + '</span> ';
        var input = document.createElement('input');
        input.className = 'annot-text';
        input.placeholder = 'Comment...';
        input.addEventListener('blur', function () {
            if (input.value.trim()) {
                annotations[key] = { type: type.id, text: input.value.trim(), path: path, line: +lineNumber };
            } else {
                delete annotations[key];
                annotation.remove();
            }
            updateExportButton();
        });
        input.addEventListener('keydown', function (event) {
            if (event.key === 'Enter') input.blur();
            if (event.key === 'Escape') { input.value = ''; input.blur(); }
        });
        annotation.appendChild(input);
        lineEl.after(annotation);
        input.focus();
    }

    function exportNotes() {
        var items = Object.keys(annotations).map(function (key) { return annotations[key]; });
        if (!items.length) { alert('No annotations to export.'); return; }
        items.sort(function (a, b) {
            if (a.path !== b.path) return a.path < b.path ? -1 : 1;
            return a.line - b.line;
        });

        var usedTypes = {};
        items.forEach(function (item) { usedTypes[item.type] = true; });
        var legend = TYPES.filter(function (type) { return usedTypes[type.id]; })
            .map(function (type) { return type.label + ' (' + type.def + ')'; })
            .join(', ');

        var out = [];
        out.push('I reviewed your code and have the following comments. Please address them.');
        out.push('');
        out.push('Comment types: ' + legend);
        out.push('');
        out.push('## Local tuicr Comments');
        out.push('');
        items.forEach(function (item, index) {
            out.push((index + 1) + '. **[' + item.type.toUpperCase() + ']** `' + item.path + ':' + item.line + '` - ' + item.text);
        });

        navigator.clipboard.writeText(out.join('\n')).then(function () { flashExport(items.length); });
    }

    function flashExport(noteCount) {
        var button = document.querySelector('.command-pane [data-export]') || document.getElementById('anno-export-btn');
        if (!button) return;
        var original = button.dataset.origLabel || button.textContent;
        button.dataset.origLabel = original;
        button.textContent = 'Copied ' + noteCount + ' notes!';
        setTimeout(function () { button.textContent = original; }, 1800);
    }

    function count() { return Object.keys(annotations).length; }

    function updateExportButton() {
        var button = document.getElementById('anno-export-btn');
        if (button) {
            var current = count();
            button.textContent = 'Export ' + current + ' note' + (current === 1 ? '' : 's');
            button.style.display = current > 0 ? 'block' : 'none';
        }
        var paneButton = document.querySelector('.command-pane [data-export]');
        if (paneButton && !paneButton.dataset.origLabel) {
            paneButton.dataset.origLabel = paneButton.textContent;
        }
    }

    function ensureFloatingButton() {
        if (document.querySelector('.command-pane')) return;
        if (document.getElementById('anno-export-btn')) return;
        if (!document.querySelector('pre code[data-path]')) return;
        var button = document.createElement('button');
        button.id = 'anno-export-btn';
        button.className = 'anno-export-btn';
        button.style.display = 'none';
        button.onclick = exportNotes;
        document.body.appendChild(button);
    }

    function initAll() {
        document.querySelectorAll('pre code[data-path]').forEach(initBlock);
        ensureFloatingButton();
        updateExportButton();
    }

    window.Annotate = { initAll: initAll, exportNotes: exportNotes, count: count };

    // Registered inside DOMContentLoaded so document.body exists even when this
    // script is loaded from <head>.
    document.addEventListener('DOMContentLoaded', function () {
        initAll();
        document.body.addEventListener('htmx:afterSwap', initAll);
    });
})();
