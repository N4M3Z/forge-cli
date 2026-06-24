// Renders markdown with reference-style links turned into footnotes.
//
// Plain marked.parse leaves two things invisible or broken in a read-only
// preview: a trailing block of `[label]: url "title"` definitions renders as
// nothing, and a `[text][label]` whose label has no definition renders as raw
// brackets. Both carry the "more information" a reader wants. This renderer
// keeps the link text inline with a superscript marker and collects every
// target into a visible "References" list at the end.
(function () {
    function escapeHtml(value) {
        return value
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;");
    }

    function looksLikeTarget(label) {
        return /[/.]/.test(label) || /^https?:/i.test(label);
    }

    window.renderMarkdownWithFootnotes = function (source) {
        // Protect fenced and inline code so link rewriting never touches it.
        var protectedBlocks = [];
        source = source.replace(/```[\s\S]*?```|`[^`\n]*`/g, function (match) {
            protectedBlocks.push(match);
            return "@@FORGEPROT" + (protectedBlocks.length - 1) + "@@";
        });

        // Collect and remove reference definitions: [label]: url "title".
        var definitions = {};
        source = source.replace(
            /^[ ]{0,3}\[([^\]]+)\]:[ \t]+(\S+)(?:[ \t]+(?:"([^"]*)"|'([^']*)'|\(([^)]*)\)))?[ \t]*$/gm,
            function (_match, label, url, doubleTitle, singleTitle, parenTitle) {
                definitions[label.toLowerCase()] = {
                    url: url,
                    title: doubleTitle || singleTitle || parenTitle || "",
                };
                return "";
            }
        );

        var notes = [];
        function noteNumber(url, title) {
            for (var index = 0; index < notes.length; index++) {
                if (notes[index].url === url) {
                    return index + 1;
                }
            }
            notes.push({ url: url, title: title || "" });
            return notes.length;
        }
        function marker(text, url, title) {
            var number = noteNumber(url, title);
            return (
                text +
                '<sup class="fn-ref"><a href="#fn-' +
                number +
                '">' +
                number +
                "</a></sup>"
            );
        }

        // Full reference links: [text][label].
        source = source.replace(
            /\[([^\]]+)\]\[([^\]]*)\]/g,
            function (match, text, label) {
                var key = (label || text).toLowerCase();
                if (definitions[key]) {
                    return marker(text, definitions[key].url, definitions[key].title);
                }
                if (looksLikeTarget(label)) {
                    return marker(text, label, "");
                }
                return match;
            }
        );

        // Shortcut references: [label] standing alone, only when defined.
        source = source.replace(/\[([^\]]+)\](?![([:])/g, function (match, label) {
            var key = label.toLowerCase();
            if (definitions[key]) {
                return marker(label, definitions[key].url, definitions[key].title);
            }
            return match;
        });

        source = source.replace(/@@FORGEPROT(\d+)@@/g, function (_match, index) {
            return protectedBlocks[+index];
        });

        var html = marked.parse(source);

        if (notes.length) {
            var items = notes
                .map(function (note, index) {
                    var number = index + 1;
                    var target = /^https?:\/\//i.test(note.url)
                        ? '<a href="' +
                          escapeHtml(note.url) +
                          '" rel="noopener noreferrer">' +
                          escapeHtml(note.url) +
                          "</a>"
                        : '<span class="fn-path">' + escapeHtml(note.url) + "</span>";
                    var title = note.title
                        ? ' <span class="fn-note">' + escapeHtml(note.title) + "</span>"
                        : "";
                    return '<li id="fn-' + number + '">' + target + title + "</li>";
                })
                .join("");
            html +=
                '<section class="md-footnotes"><div class="fn-title">References</div><ol>' +
                items +
                "</ol></section>";
        }

        return html;
    };
})();
