//! The **shared reading theme** (spec §5 — "one reviewed template", the shared visual language). One
//! self-contained stylesheet + a little inline JS, embedded in every page: **no external CDN, font,
//! or asset is fetched** (the site is fully offline-capable — every byte is inlined here). The theme
//! is tuned for *comfortable, low-fatigue reading* (the pedagogically-critical requirement): a
//! legible measure, generous line-height, soft light/dark palettes (never stark `#000`/`#fff`), a
//! persistent navigation sidebar, an "on this page" table of contents, comfortable tables with
//! overflow scrolling, and calm lexical syntax colours.
//!
//! ## One stylesheet, many renderers
//! [`READING_CSS`] is the single source of truth for the corpus site ([`crate::emit::html`]) *and*
//! the book ([`crate::book`]) — both embed the same string, so they share one visual language (§5).
//! It carries the selectors both need (the corpus sidebar/search *and* the book pager/breadcrumb);
//! unused selectors on a given page are harmless and keep the theme DRY (one const, not two).
//!
//! ## Light / dark, never-silent
//! The palette follows `prefers-color-scheme` by default and a **persisted toggle** overrides it
//! (`data-theme` on `:root`, remembered in `localStorage`). [`HEAD_THEME_INIT`] runs in `<head>`
//! *before* first paint so there is no flash of the wrong theme. Both palettes are tuned for reading
//! comfort and sufficient contrast (WCAG AA target — `Empirical/Declared`: chosen for legibility, not
//! verified by an automated contrast checker in this crate; the §4.1 legibility lint checks the
//! *structural* aspects — semantic landmarks, labelled nav, heading order — colour-contrast remains
//! its honestly-named dormant sub-aspect).

/// The shared, self-contained stylesheet (the one reviewed template's CSS, §5). Embedded inline in
/// every page; fetches nothing. Feeds the pinned template hash recorded in each corpus-page footer
/// (provenance, §6) via [`crate::emit::html::template_hash`].
///
/// **Design system (reviewed reference).** The palette is Mycelium's guarantee lattice made visual —
/// `moss` = Exact/Proven (primary accent, active nav), `amber` = Empirical (the "✓ checked" badge),
/// `clay` = Declared (dead-xref strike, warnings), `ink-blue` = links/strings. Components are styled
/// **through the tokens**, never with ad-hoc colours. Light/dark are declared so the persisted toggle
/// wins in both directions (`:root[data-theme="…"]` overrides the `prefers-color-scheme` default).
pub const READING_CSS: &str = r#"
:root{
  --paper:#ecefe4;--paper-2:#e0e5d7;--ink:#161911;--ink-soft:#353b2e;--dim:#5a6151;
  --line:#cfd5c4;--line-soft:#dbe0d0;--moss:#386f4e;--moss-deep:#235035;--amber:#9c6c17;
  --clay:#a94a34;--ink-blue:#345872;--code-bg:#e4e9d9;--code-edge:#cfd5c1;
  --tok-kw:#235035;--tok-type:#6f5210;--tok-num:#96422b;--tok-str:#345872;
  --tok-com:#767d6b;--tok-fn:#1f241b;--tok-op:#545b47;
  --serif:"Iowan Old Style","Charter","Palatino Linotype",Palatino,Georgia,serif;
  --sans:ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;
  --mono:ui-monospace,"Cascadia Code","JetBrains Mono","SF Mono","Source Code Pro",Menlo,Consolas,monospace;
  --measure:68ch;--sidebar-w:16rem;--toc-w:14rem;
}
:root[data-theme="light"]{
  --paper:#ecefe4;--paper-2:#e0e5d7;--ink:#161911;--ink-soft:#353b2e;--dim:#5a6151;
  --line:#cfd5c4;--line-soft:#dbe0d0;--moss:#386f4e;--moss-deep:#235035;--amber:#9c6c17;
  --clay:#a94a34;--ink-blue:#345872;--code-bg:#e4e9d9;--code-edge:#cfd5c1;
  --tok-kw:#235035;--tok-type:#6f5210;--tok-num:#96422b;--tok-str:#345872;
  --tok-com:#767d6b;--tok-fn:#1f241b;--tok-op:#545b47;
}
:root[data-theme="dark"]{
  --paper:#14160f;--paper-2:#1b1e16;--ink:#e6e9dd;--ink-soft:#c3c8b6;--dim:#8b917f;
  --line:#2b2f24;--line-soft:#23271d;--moss:#74c091;--moss-deep:#59a074;--amber:#d9a648;
  --clay:#e08a6f;--ink-blue:#86b4cf;--code-bg:#1b1e16;--code-edge:#2b2f24;
  --tok-kw:#74c091;--tok-type:#d9a648;--tok-num:#e5926f;--tok-str:#86b4cf;
  --tok-com:#7f8873;--tok-fn:#e6e9dd;--tok-op:#a9b09a;
}
@media (prefers-color-scheme: dark){
  :root{
    --paper:#14160f;--paper-2:#1b1e16;--ink:#e6e9dd;--ink-soft:#c3c8b6;--dim:#8b917f;
    --line:#2b2f24;--line-soft:#23271d;--moss:#74c091;--moss-deep:#59a074;--amber:#d9a648;
    --clay:#e08a6f;--ink-blue:#86b4cf;--code-bg:#1b1e16;--code-edge:#2b2f24;
    --tok-kw:#74c091;--tok-type:#d9a648;--tok-num:#e5926f;--tok-str:#86b4cf;
    --tok-com:#7f8873;--tok-fn:#e6e9dd;--tok-op:#a9b09a;
  }
}
*{box-sizing:border-box}
html{scroll-behavior:smooth}
body{
  margin:0;font-family:var(--serif);font-size:1.0625rem;line-height:1.68;
  color:var(--ink);background:var(--paper);
  -webkit-font-smoothing:antialiased;text-rendering:optimizeLegibility;
}
::selection{background:color-mix(in srgb,var(--moss) 22%,transparent)}
a{color:var(--ink-blue);text-decoration-thickness:.06em;text-underline-offset:.15em}
a:hover{text-decoration-color:var(--moss)}
a.unresolved{color:var(--clay);text-decoration:line-through}
:focus-visible{outline:2px solid var(--moss);outline-offset:2px;border-radius:3px}
.skip-link{position:absolute;left:-999px;top:0;background:var(--moss);color:#fff;
  font-family:var(--sans);padding:.5rem .9rem;border-radius:0 0 6px 0;z-index:20}
.skip-link:focus{left:0}
/* page frame */
.site-header{border-bottom:1px solid var(--line);background:var(--paper-2);
  position:sticky;top:0;z-index:10}
.site-header .bar{max-width:82rem;margin:0 auto;padding:.7rem 1.25rem;
  display:flex;align-items:center;gap:1rem}
.site-header .site-title{font-family:var(--sans);font-size:1.02rem;margin:0;font-weight:650;
  letter-spacing:-.01em}
.site-header .tagline{font-family:var(--sans);color:var(--dim);font-size:.82rem;margin:0;flex:1;
  min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.theme-toggle{cursor:pointer;border:1px solid var(--line);background:var(--paper);
  color:var(--ink);border-radius:8px;padding:.35rem .6rem;font-size:.95rem;line-height:1}
.theme-toggle:hover{border-color:var(--moss)}
.layout{max-width:82rem;margin:0 auto;display:grid;
  grid-template-columns:var(--sidebar-w) minmax(0,1fr) var(--toc-w);
  gap:1.6rem;padding:0 1.25rem;align-items:start}
.layout.no-toc{grid-template-columns:var(--sidebar-w) minmax(0,1fr)}
.sidebar,.on-this-page{position:sticky;top:4.2rem;max-height:calc(100vh - 5rem);
  overflow:auto;padding:1.3rem 0;font-family:var(--sans);font-size:.9rem}
.sidebar{border-right:1px solid var(--line)}
main{padding:1.5rem 0 3.5rem;min-width:0;max-width:var(--measure);margin:0 auto}
/* sidebar nav tree */
.nav-search{width:100%;padding:.5rem .6rem;font-family:var(--sans);font-size:.9rem;
  border:1px solid var(--line);border-radius:8px;background:var(--paper);color:var(--ink);
  margin-bottom:.6rem}
.nav-search:focus{outline:2px solid var(--moss);border-color:var(--moss)}
.search-results{list-style:none;margin:.3rem 0 .8rem;padding:0}
.search-results li{margin:.15rem 0}
.search-results a{display:block;padding:.25rem .4rem;border-radius:6px;text-decoration:none;
  color:var(--ink)}
.search-results a:hover{background:var(--paper-2)}
.search-results .hint,.search-results .empty{color:var(--dim);font-size:.82rem;padding:.25rem .4rem}
.nav-group{margin:.95rem 0 .2rem;font-size:.7rem;font-weight:700;letter-spacing:.1em;
  text-transform:uppercase;color:var(--dim)}
.sidebar ul{list-style:none;margin:0;padding:0}
/* collapsible family/topic groups (native details/summary — no JS) */
.sidebar details{margin:.05rem 0}
.sidebar summary{cursor:pointer;list-style:none;display:flex;align-items:center;gap:.35rem;
  padding:.28rem .4rem;border-radius:6px;font-size:.82rem;font-weight:600;color:var(--ink-soft)}
.sidebar summary::-webkit-details-marker{display:none}
.sidebar summary::before{content:"\25b8";color:var(--dim);font-size:.7em;display:inline-block;
  transition:transform .12s}
.sidebar details[open]>summary::before{transform:rotate(90deg)}
.sidebar summary:hover{background:var(--paper-2);color:var(--ink)}
.sidebar summary:focus-visible{outline:2px solid var(--moss);outline-offset:1px}
.sidebar .count{margin-left:auto;font-size:.72rem;font-weight:500;color:var(--dim);
  font-variant-numeric:tabular-nums}
.sidebar details>ul{margin:.05rem 0 .3rem .55rem;padding-left:.35rem;
  border-left:1px solid var(--line-soft)}
.sidebar li a{display:block;padding:.15rem .45rem;border-radius:6px;color:var(--ink-soft);
  text-decoration:none;border-left:2px solid transparent;font-size:.82rem;line-height:1.35;
  white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.sidebar li a:hover{background:var(--paper-2);color:var(--ink)}
.sidebar li a[aria-current="page"]{color:var(--moss-deep);font-weight:600;
  border-left-color:var(--moss);background:var(--paper-2)}
/* on this page */
.toc-title,.nav-title{font-size:.7rem;font-weight:700;letter-spacing:.1em;text-transform:uppercase;
  color:var(--dim);margin:0 0 .4rem}
.on-this-page ul{list-style:none;margin:0;padding:0}
.on-this-page a{display:block;padding:.16rem 0;color:var(--dim);text-decoration:none;
  border-left:2px solid var(--line);padding-left:.7rem}
.on-this-page a:hover{color:var(--ink);border-left-color:var(--moss)}
.on-this-page .lvl-3{padding-left:1.4rem}
.on-this-page .lvl-4{padding-left:2.1rem}
/* reading typography (serif body + headings) */
h1,h2,h3,h4,h5,h6{font-family:var(--serif);line-height:1.24;font-weight:600;
  margin:1.9em 0 .55em;scroll-margin-top:4.6rem}
h1{font-size:2.15rem;margin-top:.2em;letter-spacing:-.018em;text-wrap:balance}
h2{font-size:1.5rem;margin-top:1.7em;padding-top:.9rem;border-top:1px solid var(--line-soft)}
h3{font-size:1.22rem}h4{font-size:1.06rem}h5,h6{font-size:.96rem;color:var(--ink-soft)}
p{margin:0 0 1.05em}
ul,ol{margin:0 0 1.05em;padding-left:1.4em}li{margin:.28em 0}
main>article>section{margin-bottom:.5rem}
blockquote{margin:1.2em 0;padding:.2em 1em;border-left:3px solid var(--line);color:var(--ink-soft)}
hr{border:0;border-top:1px solid var(--line);margin:2em 0}
/* code */
code{font-family:var(--mono);font-size:.86em}
:not(pre)>code{background:var(--code-bg);padding:.12em .35em;border-radius:5px;
  border:1px solid var(--code-edge)}
pre{background:var(--code-bg);border:1px solid var(--code-edge);padding:.85rem 1rem;
  border-radius:10px;overflow-x:auto;line-height:1.6;margin:0 0 1.15em}
pre code{background:none;border:0;padding:0;font-size:.86rem}
figure{margin:0 0 1.25em}
figure>.checked,figure>.level{display:inline-flex;align-items:center;margin-bottom:.4rem}
/* syntax tokens (lexical only — Empirical/Declared; only fn-position idents are distinguished) */
.tok-kw{color:var(--tok-kw);font-weight:600}
.tok-type{color:var(--tok-type)}
.tok-num{color:var(--tok-num)}
.tok-str{color:var(--tok-str)}
.tok-com{color:var(--tok-com);font-style:italic}
.tok-fn{color:var(--tok-fn);font-weight:600}
.tok-op{color:var(--tok-op)}
/* comfortable tables (the corpus is table-heavy) — own scroll container */
.table-wrap{overflow-x:auto;margin:0 0 1.25em;border:1px solid var(--line);border-radius:10px}
table{border-collapse:collapse;width:100%;font-size:.95rem}
thead th{font-family:var(--sans);background:var(--paper-2);text-align:left;font-weight:640;
  font-size:.86rem;letter-spacing:.01em}
th,td{padding:.5rem .8rem;border-bottom:1px solid var(--line);vertical-align:top}
tbody tr:nth-child(even){background:var(--paper-2)}
tbody tr:last-child td{border-bottom:0}
/* guarantee chips + markers (pill, sans, leading dot, lattice-tinted) */
.level,.checked{font-family:var(--sans);font-size:.72rem;line-height:1.4;border-radius:999px;
  padding:.06rem .55rem .06rem .5rem;letter-spacing:.02em;text-transform:none}
.level::before,.checked::before{content:"\2022";margin-right:.32rem;font-size:.9em}
.level{color:var(--dim);background:color-mix(in srgb,var(--dim) 12%,transparent);
  border:1px solid color-mix(in srgb,var(--dim) 34%,transparent)}
.checked{color:var(--amber);background:color-mix(in srgb,var(--amber) 13%,transparent);
  border:1px solid color-mix(in srgb,var(--amber) 40%,transparent)}
.undocumented{color:var(--ink-soft);font-style:italic;border-left:3px solid var(--clay);
  padding-left:.6rem}
/* book chrome */
.crumb{font-family:var(--sans);font-size:.85rem;color:var(--dim);margin:.2rem 0 1rem}
.pager{display:flex;justify-content:space-between;gap:1rem;margin:2.6rem 0 0;
  font-family:var(--sans);font-size:.9rem}
.pager a{border:1px solid var(--line);border-radius:10px;padding:.55rem .9rem;
  text-decoration:none;color:var(--ink);background:var(--paper-2)}
.pager a:hover{border-color:var(--moss)}
#book-search-box{width:100%;padding:.6rem .7rem;font-family:var(--sans);font-size:1rem;
  border:1px solid var(--line);border-radius:10px;background:var(--paper);color:var(--ink)}
#book-search-results{list-style:none;padding:0}#book-search-results li{margin:.5rem 0}
footer{font-family:var(--sans);color:var(--dim);font-size:.84rem;border-top:1px solid var(--line);
  margin:2.5rem auto 0;padding:1.2rem 1.25rem;max-width:82rem}
/* responsive: fold the rails away on narrow screens */
@media (max-width:60rem){
  .layout,.layout.no-toc{grid-template-columns:minmax(0,1fr)}
  .on-this-page{display:none}
  .sidebar{position:static;max-height:none;border-right:0;border-bottom:1px solid var(--line);top:0}
}
@media (prefers-reduced-motion: reduce){html{scroll-behavior:auto}}
"#;

/// The `<head>` theme bootstrap — runs **before first paint** so the correct palette is applied with
/// no flash-of-wrong-theme. Reads the persisted choice; when unset it leaves `data-theme` off so the
/// CSS `prefers-color-scheme` default governs (never-silent: an unreadable `localStorage` is caught
/// and simply falls back to the media-query default).
pub const HEAD_THEME_INIT: &str = r#"<script>
(function(){try{var t=localStorage.getItem('myc-theme');
if(t==='dark'||t==='light'){document.documentElement.setAttribute('data-theme',t);}}
catch(e){}})();
</script>"#;

/// The theme-toggle button (goes in the header bar). Wired by [`THEME_TOGGLE_JS`].
pub const THEME_TOGGLE_BUTTON: &str =
    "<button class=\"theme-toggle\" id=\"theme-toggle\" type=\"button\" \
     aria-label=\"Toggle light or dark theme\" title=\"Toggle light/dark\">\u{25d0}</button>";

/// Wires [`THEME_TOGGLE_BUTTON`]: flips the *effective* theme (honouring the OS default when no
/// choice is stored yet) and persists it. Placed at end-of-body.
pub const THEME_TOGGLE_JS: &str = r#"<script>
(function(){
  var btn=document.getElementById('theme-toggle');
  if(!btn)return;
  function effective(){
    var a=document.documentElement.getAttribute('data-theme');
    if(a)return a;
    return window.matchMedia&&window.matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';
  }
  btn.addEventListener('click',function(){
    var next=effective()==='dark'?'light':'dark';
    document.documentElement.setAttribute('data-theme',next);
    try{localStorage.setItem('myc-theme',next);}catch(e){}
  });
})();
</script>"#;

/// The corpus-site client-side search: a small substring/prefix filter over the already-emitted
/// `search-index.jsonl` (one JSON record per node — [`crate::emit::json`]). No new dependency, no
/// search engine; fetched relative to `window.MYC_BASE` (`""` on the index, `"../"` on a page).
///
/// Never-silent (G2): a failed `fetch` (e.g. opened over `file://` where the browser blocks it)
/// shows an explicit hint rather than a silently-dead box — the honest degradation, the same posture
/// as the rest of this crate. Only records that carry a `title` (documents/sections — the meaningful
/// navigation targets) are surfaced; matching is case-insensitive over title + anchor.
pub const CORPUS_SEARCH_JS: &str = r#"<script>
(function(){
  var box=document.getElementById('corpus-search-box');
  var out=document.getElementById('corpus-search-results');
  if(!box||!out)return;
  var base=window.MYC_BASE||'';
  var records=null,loadErr=false;
  function docOf(a){var i=a.indexOf('--');return i<0?a:a.slice(0,i);}
  function load(){
    return fetch(base+'search-index.jsonl').then(function(r){return r.text();}).then(function(t){
      records=t.split('\n').filter(Boolean).map(function(l){try{return JSON.parse(l);}catch(e){return null;}})
        .filter(function(r){return r&&r.title;});
    }).catch(function(){loadErr=true;});
  }
  function render(q){
    out.innerHTML='';
    if(loadErr){out.innerHTML='<li class="empty">Search needs the site served over http:// (fetch is blocked on file://).</li>';return;}
    if(records===null){out.innerHTML='<li class="hint">Loading index…</li>';return;}
    q=(q||'').trim().toLowerCase();
    if(!q){return;}
    var hits=records.filter(function(r){
      return r.title.toLowerCase().indexOf(q)>=0||(r.anchor||'').toLowerCase().indexOf(q)>=0;
    }).slice(0,40);
    if(hits.length===0){out.innerHTML='<li class="empty">No matches.</li>';return;}
    hits.forEach(function(r){
      var li=document.createElement('li');var a=document.createElement('a');
      a.href=base+'pages/'+docOf(r.anchor)+'.html#'+r.anchor;
      a.textContent=r.title;a.setAttribute('data-kind',r.kind||'');
      li.appendChild(a);out.appendChild(li);
    });
  }
  box.addEventListener('input',function(){
    if(records===null&&!loadErr){load().then(function(){render(box.value);});}
    else{render(box.value);}
  });
  box.addEventListener('focus',function(){if(records===null&&!loadErr){load();}});
})();
</script>"#;

/// The skip-to-content link (accessibility — first focusable element).
pub const SKIP_LINK: &str = "<a class=\"skip-link\" href=\"#content\">Skip to content</a>";
