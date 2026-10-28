// Shared behavior of the Ajisai Reference (public/docs/ja/index.html and
// public/docs/en/index.html). Both pages load this file with `defer`, so the
// DOM is ready when it runs.

document.getElementById('footer-year').textContent = new Date().getFullYear();

// Mathematics is LaTeX typeset by the self-hosted KaTeX, loaded (deferred)
// before this script. Only \( ... \) and \[ ... \] delimit it; neither
// collides with any Ajisai surface form, and code/pre are never scanned, so
// the Ajisai channel is never typeset (docs/dev/ajisai-authoring-style.md §4).
if (typeof renderMathInElement === 'function') {
    renderMathInElement(document.body, {
        delimiters: [
            { left: '\\(', right: '\\)', display: false },
            { left: '\\[', right: '\\]', display: true }
        ],
        ignoredTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code'],
        throwOnError: false
    });
}

// Language switch: the selected option is always the language of the page
// being shown (<html lang>). Choosing another language opens that version at
// the same #section, so the hash is carried over; the select is re-synced on
// hashchange too, because the browser may restore a stale selection.
(function () {
    var langSelect = document.getElementById('lang-select');
    if (!langSelect) return;
    var current = document.documentElement.lang;
    function syncLangSelect() {
        langSelect.value = current;
    }
    langSelect.addEventListener('change', function () {
        if (langSelect.value === current) return;
        window.location.href = '../' + langSelect.value + '/index.html' + window.location.hash;
    });
    window.addEventListener('hashchange', syncLangSelect);
    window.addEventListener('pageshow', syncLangSelect);
    syncLangSelect();
})();

// Each sample's "Open in Playground" link gets a URL carrying the sample's
// code (the first row's sample-code cell). The app reads #code=<encoded> and
// loads it into the editor (gui-application.ts: applyPlaygroundCodeFromUrl).
document.querySelectorAll('.js-open-playground').forEach(function (link) {
    var sample = link.closest('.sample');
    var codeEl = sample && sample.querySelector('.ref-table tbody tr td:first-child code');
    if (!codeEl) return;
    var code = codeEl.textContent.replace(/\s+$/, '');
    link.setAttribute('href', '../../index.html#code=' + encodeURIComponent(code));
});

// Each navigation item is one page (a section in the article). Only the
// selected page is shown. The URL hash names the current page (e.g. #vectors),
// so back/forward and bookmarks work.
(function () {
    var pages = Array.prototype.slice.call(document.querySelectorAll('.ref-page'));
    var navLinks = Array.prototype.slice.call(
        document.querySelectorAll('.ref-nav a[href^="#"]')
    );
    if (pages.length === 0) return;

    var mobileQuery = window.matchMedia('(max-width: 768px)');
    // On mobile only this content area scrolls (the body is fixed).
    var contentArea = document.querySelector('.ref-main');

    function pageIdFromLink(link) {
        return (link.getAttribute('href') || '').replace(/^#/, '');
    }

    // The page a hash points at. A hash may name a page id itself, or an id
    // inside a page (a heading kept from a merged page, or a Word entry of the
    // Word list); either resolves to the page that contains it.
    function pageFromHash() {
        var id = decodeURIComponent(window.location.hash.replace(/^#/, ''));
        if (!id) return null;
        for (var i = 0; i < pages.length; i++) {
            if (pages[i].id === id) return pages[i];
        }
        var target = document.getElementById(id);
        return target ? target.closest('.ref-page') : null;
    }

    function currentPageIndex() {
        var page = pageFromHash();
        return page ? pages.indexOf(page) : 0;
    }

    // Move to the previous or next page (nothing at either end). Writing the
    // hash fires hashchange, and syncFromHash switches the page and scrolls.
    function goToPageDelta(delta) {
        var next = currentPageIndex() + delta;
        if (next < 0 || next >= pages.length) return;
        window.location.hash = '#' + pages[next].id;
    }

    // Page visibility itself is handled in CSS via :target / :has(); JS only
    // mirrors the current page into aria-current (accessibility; CSS cannot
    // set it), falling back to the first page when nothing is targeted.
    function markCurrentPage() {
        var target = pageFromHash() || pages[0];

        navLinks.forEach(function (link) {
            if (pageIdFromLink(link) === target.id) {
                link.setAttribute('aria-current', 'page');
            } else {
                link.removeAttribute('aria-current');
            }
        });
    }

    function syncFromHash(userInitiated) {
        markCurrentPage();
        // On mobile, a page switch returns to the top so the new page reads
        // from its start. A hash naming something inside a page is left to the
        // browser, which scrolls to it.
        var hashId = window.location.hash.replace(/^#/, '');
        var isPage = pages.some(function (page) { return page.id === hashId; });
        if (userInitiated && isPage && mobileQuery.matches && contentArea) {
            contentArea.scrollTo({ top: 0, behavior: 'smooth' });
        }
    }

    window.addEventListener('hashchange', function () { syncFromHash(true); });
    syncFromHash(false);

    // Mobile: a horizontal swipe also moves between pages — left for next,
    // right for previous. A swipe that starts on a horizontally scrollable
    // table is left to the table's own scrolling.
    (function () {
        if (!contentArea) return;
        var SWIPE_MIN_X = 60;   // horizontal travel that counts as a swipe
        var startX = 0, startY = 0, tracking = false;

        contentArea.addEventListener('touchstart', function (e) {
            if (e.touches.length !== 1 || e.target.closest('.ref-table-wrap')) {
                tracking = false;
                return;
            }
            startX = e.touches[0].clientX;
            startY = e.touches[0].clientY;
            tracking = true;
        }, { passive: true });

        contentArea.addEventListener('touchend', function (e) {
            if (!tracking) return;
            tracking = false;
            var dx = e.changedTouches[0].clientX - startX;
            var dy = e.changedTouches[0].clientY - startY;
            // React only when the horizontal travel is large enough and
            // clearly larger than the vertical.
            if (Math.abs(dx) < SWIPE_MIN_X || Math.abs(dx) < Math.abs(dy) * 1.5) return;
            goToPageDelta(dx < 0 ? 1 : -1);
        }, { passive: true });
    })();
})();
