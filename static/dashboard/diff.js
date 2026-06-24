// Shared line diff (LCS) used by the deployed-file viewer and the artifact
// Diff tab. `render(bodyEl, aText, bText)` shows aText as additions (+) and
// bText as deletions (-).
(function () {
    function escapeHtml(text) {
        return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    function diffLines(a, b) {
        var n = a.length;
        var m = b.length;
        var lcs = Array.from({ length: n + 1 }, function () {
            return new Array(m + 1).fill(0);
        });
        for (var i = n - 1; i >= 0; i--) {
            for (var j = m - 1; j >= 0; j--) {
                lcs[i][j] = a[i] === b[j]
                    ? lcs[i + 1][j + 1] + 1
                    : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
            }
        }
        var rows = [];
        var i2 = 0;
        var j2 = 0;
        while (i2 < n && j2 < m) {
            if (a[i2] === b[j2]) {
                rows.push({ tag: ' ', text: a[i2] });
                i2++;
                j2++;
            } else if (lcs[i2 + 1][j2] >= lcs[i2][j2 + 1]) {
                rows.push({ tag: '+', text: a[i2] });
                i2++;
            } else {
                rows.push({ tag: '-', text: b[j2] });
                j2++;
            }
        }
        while (i2 < n) {
            rows.push({ tag: '+', text: a[i2++] });
        }
        while (j2 < m) {
            rows.push({ tag: '-', text: b[j2++] });
        }
        return rows;
    }

    function render(bodyEl, aText, bText) {
        if (!bodyEl) return;
        var rows = diffLines((aText || '').split('\n'), (bText || '').split('\n'));
        bodyEl.innerHTML = rows.map(function (row) {
            var cls = row.tag === '+' ? 'diff-add' : row.tag === '-' ? 'diff-del' : 'diff-ctx';
            var marker = row.tag === ' ' ? ' ' : row.tag;
            return '<span class="diff-row ' + cls + '"><span class="diff-marker">' + marker
                + '</span><span class="diff-text">' + escapeHtml(row.text) + '</span></span>';
        }).join('');
    }

    window.DashDiff = { render: render, lines: diffLines, escape: escapeHtml };
})();
