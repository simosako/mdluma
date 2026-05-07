function handleDocumentReady() {
  initializeInteractions();
}

function requestOpenFile() {
  if (typeof Window === "undefined" || !Window.this || typeof Window.this.xcall !== "function") {
    return;
  }

  Window.this.xcall("open-file-requested");
}

function openFileButton() {
  if (!document || typeof document.querySelector !== "function") {
    return null;
  }

  return document.querySelector('[data-action="open-file"]');
}

function requestThemeToggle() {
  if (typeof Window === "undefined" || !Window.this || typeof Window.this.xcall !== "function") {
    return;
  }
  Window.this.xcall("theme-toggle-requested");
}

function requestExternalEditor() {
  if (typeof Window === "undefined" || !Window.this || typeof Window.this.xcall !== "function") {
    return;
  }

  Window.this.xcall("external-editor-requested");
}

function requestExternalEditorSetting() {
  if (typeof Window === "undefined" || !Window.this || typeof Window.this.xcall !== "function") {
    return;
  }

  Window.this.xcall("external-editor-setting-requested");
}

function requestOpenRecentFile(index) {
  if (typeof Window === "undefined" || !Window.this || typeof Window.this.xcall !== "function") {
    return;
  }

  Window.this.xcall("open-recent-file", String(index));
}

function requestErrorDismiss() {
  if (typeof Window === "undefined" || !Window.this || typeof Window.this.xcall !== "function") {
    return;
  }

  Window.this.xcall("error-dismiss-requested");
}

function recentFilesMenu() {
  if (!document || typeof document.getElementById !== "function") {
    return null;
  }

  return document.getElementById("recent-files-menu");
}

function markdownBodyHost() {
  if (!document || typeof document.querySelector !== "function") {
    return null;
  }

  return document.querySelector("[data-markdown-body-host]");
}

function hasLoadedDocument() {
  const host = markdownBodyHost();
  return !!(host && typeof host.querySelector === "function" && host.querySelector("[data-document-loaded]"));
}

function normalizeEventTarget(target) {
  if (!target) {
    return null;
  }

  if (typeof target.closest === "function") {
    return target;
  }

  if (target.parentElement && typeof target.parentElement.closest === "function") {
    return target.parentElement;
  }

  if (target.parentNode && typeof target.parentNode.closest === "function") {
    return target.parentNode;
  }

  return null;
}

function isWithinMarkdownBodyHost(target) {
  const elementTarget = normalizeEventTarget(target);
  if (!elementTarget || typeof elementTarget.closest !== "function") {
    return false;
  }

  return !!(
    elementTarget.closest("[data-markdown-body-host]") ||
    elementTarget.closest("[data-markdown-body]") ||
    elementTarget.closest("[data-markdown-selection-host]")
  );
}

function markdownContextMenuHtml() {
  return `<li name="edit:copy">Copy</li><li name="edit:selectall">Select All</li><hr/><li class="external-editor" data-action="external-editor"${hasLoadedDocument() ? "" : " disabled"}>External Editor</li>`;
}

function createMarkdownContextMenu() {
  if (typeof document === "undefined" || typeof document.createElement !== "function") {
    return null;
  }

  const menu = document.createElement("menu");
  if (!menu) {
    return null;
  }

  menu.className = "context";
  menu.innerHTML = markdownContextMenuHtml();
  return menu;
}

function handleMarkdownContextMenu(event, target) {
  const sourceTarget = normalizeEventTarget(target || (event && event.target) || null);
  if (!isWithinMarkdownBodyHost(sourceTarget)) {
    return false;
  }

  const menu = createMarkdownContextMenu();
  if (!menu || !event) {
    return false;
  }

  event.source = menu;
  return true;
}

function isWithinRecentFilesMenu(target) {
  const elementTarget = normalizeEventTarget(target);
  if (!elementTarget || typeof elementTarget.closest !== "function") {
    return false;
  }

  return !!elementTarget.closest("#recent-files-menu");
}

function closestActionTarget(target) {
  return target && target.closest ? target.closest("[data-action]") : null;
}

function shouldIgnoreTarget(target) {
  return !target || target.disabled;
}

function searchPanel() {
  if (!document || typeof document.querySelector !== "function") {
    return null;
  }

  return document.querySelector("[data-search-panel]");
}

function searchInput() {
  if (!document || typeof document.querySelector !== "function") {
    return null;
  }

  return document.querySelector("[data-search-input]");
}

function searchInfoElement() {
  if (!document || typeof document.querySelector !== "function") {
    return null;
  }

  return document.querySelector("[data-search-info]");
}

function aboutOverlayElement() {
  if (!document || typeof document.querySelector !== "function") {
    return null;
  }

  return document.querySelector("[data-about-overlay]");
}

function errorOverlayElement() {
  if (!document || typeof document.querySelector !== "function") {
    return null;
  }

  return document.querySelector("[data-error-overlay]");
}

function errorOkButton() {
  if (!document || typeof document.querySelector !== "function") {
    return null;
  }

  return document.querySelector('[data-action="error-ok"]');
}

function setAboutOverlayOpen(open) {
  const overlay = aboutOverlayElement();
  if (!overlay) {
    return;
  }

  overlay.hidden = !open;
  if (open && typeof overlay.removeAttribute === "function") {
    overlay.removeAttribute("hidden");
  }
  if (typeof overlay.setAttribute === "function") {
    if (!open) {
      overlay.setAttribute("hidden", "hidden");
    }
  }
  if (overlay.style) {
    overlay.style.display = open ? "flex" : "none";
  }

  if (open) {
    const okButton = document.querySelector('[data-action="about-ok"]');
    if (okButton && typeof okButton.focus === "function") {
      okButton.focus();
    }
  }
}

function setErrorOverlayOpen(open) {
  const overlay = errorOverlayElement();
  if (!overlay) {
    return;
  }

  overlay.hidden = !open;
  if (open && typeof overlay.removeAttribute === "function") {
    overlay.removeAttribute("hidden");
  }
  if (typeof overlay.setAttribute === "function") {
    if (!open) {
      overlay.setAttribute("hidden", "hidden");
    }
  }
  if (overlay.style) {
    overlay.style.display = open ? "flex" : "none";
  }

  if (open) {
    const okButton = errorOkButton();
    if (okButton && typeof okButton.focus === "function") {
      okButton.focus();
    }
  }
}

function handleClick(target) {
  if (isWithinRecentFilesMenu(target)) {
    return;
  }

  const actionTarget = closestActionTarget(target);
  if (shouldIgnoreTarget(actionTarget)) {
    return;
  }

  const action = actionTarget.getAttribute("data-action");

  if (action !== "open-file") {
    if (action === "search") {
      toggleSearchPanel();
    } else if (action === "theme") {
      requestThemeToggle();
    } else if (action === "font") {
      return;
    } else if (action === "external-editor") {
      requestExternalEditor();
    } else if (action === "external-editor-setting") {
      requestExternalEditorSetting();
    } else if (action === "about") {
      setAboutOverlayOpen(true);
    } else if (action === "about-ok") {
      setAboutOverlayOpen(false);
    } else if (action === "error-ok") {
      setErrorOverlayOpen(false);
      requestErrorDismiss();
    } else if (action === "recent-file") {
      const index = actionTarget.getAttribute("data-recent-index");
      if (index !== null && index !== "") {
          requestOpenRecentFile(index);
      }
    }
    return;
  }

  requestOpenFile();
}

function markdownBody() {
  if (!document || typeof document.querySelector !== "function") {
    return null;
  }

  return document.querySelector("[data-markdown-body]");
}

function markdownSelectionOwner() {
  if (!document || typeof document.querySelector !== "function") {
    return null;
  }

  return document.querySelector("[data-markdown-selection-host]") || markdownBody();
}

let searchVisible = false;
let searchQuery = "";
let searchMatches = [];
let currentMatchIndex = -1;
let searchCaseSensitive = false;

function toggleSearchPanel() {
  const panel = searchPanel();
  const input = searchInput();
  if (!panel || !input) {
    return;
  }

  searchVisible = !searchVisible;

  if (searchVisible) {
    panel.hidden = false;
    if (typeof panel.removeAttribute === "function") {
      panel.removeAttribute("hidden");
    }
    if (panel.style) {
      panel.style.display = "block";
    }
    panel.classList.add("search-panel-visible");
    panel.setAttribute("aria-hidden", "false");
    input.focus();
    if (searchQuery) {
      input.value = searchQuery;
      performSearch();
    }
  } else {
    panel.hidden = true;
    if (typeof panel.setAttribute === "function") {
      panel.setAttribute("hidden", "hidden");
    }
    if (panel.style) {
      panel.style.display = "none";
    }
    panel.classList.remove("search-panel-visible");
    panel.setAttribute("aria-hidden", "true");
    clearSearchHighlights();
  }
}

function hideSearchPanel() {
  const panel = searchPanel();
  if (!panel) {
    return;
  }

  searchVisible = false;
  panel.hidden = true;
  if (typeof panel.setAttribute === "function") {
    panel.setAttribute("hidden", "hidden");
  }
  if (panel.style) {
    panel.style.display = "none";
  }
  panel.classList.remove("search-panel-visible");
  panel.setAttribute("aria-hidden", "true");
  clearSearchHighlights();
}

function copyStatusElement() {
  if (!document || typeof document.querySelector !== "function") {
    return null;
  }

  return document.querySelector("[data-copy-status]");
}

function showCopyFailure(message) {
  const element = copyStatusElement();
  if (!element) {
    return;
  }

  element.textContent = message;
}

function clearCopyStatus() {
  const element = copyStatusElement();
  if (!element) {
    return;
  }

  element.textContent = "";
}

function selectedText() {
  const owner = markdownSelectionOwner();
  const selection = owner && owner.selection;
  if (!selection || selection.isCollapsed || typeof selection.toString !== "function") {
    return "";
  }

  const text = selection.toString();
  if (typeof text !== "string") {
    return "";
  }

  return text;
}

function isCopyShortcut(event) {
  if (!event) {
    return false;
  }

  const key = typeof event.key === "string" ? event.key.toLowerCase() : "";
  return (event.ctrlKey || event.metaKey) && key === "c";
}

function handleCopyShortcut(event) {
  if (!isCopyShortcut(event)) {
    return;
  }

  const text = selectedText();
  if (!text || typeof Clipboard === "undefined" || typeof Clipboard.writeText !== "function") {
    return;
  }

  try {
    if (!Clipboard.writeText(text)) {
      showCopyFailure("Copy failed. Try again.");
      return;
    }
  } catch (_error) {
    showCopyFailure("Copy failed. Try again.");
    return;
  }

  clearCopyStatus();

  if (typeof event.preventDefault === "function") {
    event.preventDefault();
  }
}

let interactionsInitialized = false;
let repaintRegistrationInitialized = false;
let markdownContextMenuBound = false;

function collectTextNodes(node, textNodes) {
  if (!node) {
    return;
  }

  if (node.nodeType === Node.TEXT_NODE) {
    const text = node.textContent;
    if (text && text.trim().length > 0) {
      textNodes.push(node);
    }
  } else {
    for (let child = node.firstChild; child; child = child.nextSibling) {
      collectTextNodes(child, textNodes);
    }
  }
}

function findAllMatches(query, caseInsensitive) {
  const body = markdownBody();
  if (!body || !query) {
    return [];
  }

  const textNodes = [];
  collectTextNodes(body, textNodes);

  const matches = [];
  const searchQuery = caseInsensitive ? query.toLowerCase() : query;

  for (const textNode of textNodes) {
    const text = textNode.textContent;
    const searchText = caseInsensitive ? text.toLowerCase() : text;

    let startIndex = 0;
    while (true) {
      const index = searchText.indexOf(searchQuery, startIndex);
      if (index === -1) {
        break;
      }

      const range = new Range();
      range.setStart(textNode, index);
      range.setEnd(textNode, index + query.length);

      matches.push({
        range: range,
        textNode: textNode,
        startOffset: index,
        endOffset: index + query.length,
      });

      startIndex = index + query.length;
    }
  }

  return matches;
}

function clearSearchHighlights() {
  const body = markdownBody();
  if (!body) {
    return;
  }

  try {
    const range = new Range();
    range.selectNodeContents(body);
    range.clearMark("search");
    range.clearMark("current");
  } catch (_error) {
  }

  searchMatches = [];
  currentMatchIndex = -1;
  updateSearchInfo();
}

function applySearchHighlights() {
  const body = markdownBody();
  if (!body) {
    return;
  }

  try {
    const resetRange = new Range();
    resetRange.selectNodeContents(body);
    resetRange.clearMark("search");
    resetRange.clearMark("current");
  } catch (_error) {
  }

  for (let i = 0; i < searchMatches.length; i++) {
    const match = searchMatches[i];
    try {
      match.range.applyMark("search");
    } catch (_error) {
    }
  }

  if (currentMatchIndex >= 0 && currentMatchIndex < searchMatches.length) {
    try {
      searchMatches[currentMatchIndex].range.applyMark("current");
    } catch (_error) {
    }
  }

  updateSearchInfo();
}

function updateSearchInfo() {
  const info = searchInfoElement();
  if (!info) {
    return;
  }

  const count = searchMatches.length;
  if (count === 0 && searchQuery) {
    info.textContent = "No matches";
  } else if (count > 0) {
    const currentIndex = currentMatchIndex + 1;
    info.textContent = `${currentIndex} of ${count}`;
  } else {
    info.textContent = "";
  }
}

function performSearch() {
  const input = searchInput();
  if (!input) {
    return;
  }

  searchQuery = input.value.trim();

  if (!searchQuery) {
    clearSearchHighlights();
    return;
  }

  searchMatches = findAllMatches(searchQuery, !searchCaseSensitive);
  currentMatchIndex = searchMatches.length > 0 ? 0 : -1;

  applySearchHighlights();

  if (searchMatches.length > 0) {
    scrollToMatch(currentMatchIndex);
  }
}

function scrollToMatch(index) {
  if (index < 0 || index >= searchMatches.length) {
    return;
  }

  const match = searchMatches[index];
  try {
    applySearchHighlights();

    const startNode = match.range.startContainer;
    const target = startNode && startNode.nodeType === Node.TEXT_NODE
      ? startNode.parentElement
      : startNode;
    if (target && typeof target.scrollIntoView === "function") {
      target.scrollIntoView(true);
    }
  } catch (_error) {
  }
}

function navigateToNextMatch() {
  if (searchMatches.length === 0) {
    return;
  }

  currentMatchIndex = (currentMatchIndex + 1) % searchMatches.length;
  scrollToMatch(currentMatchIndex);
  updateSearchInfo();
}

function navigateToPrevMatch() {
  if (searchMatches.length === 0) {
    return;
  }

  currentMatchIndex = (currentMatchIndex - 1 + searchMatches.length) % searchMatches.length;
  scrollToMatch(currentMatchIndex);
  updateSearchInfo();
}

function bindDelegatedClickHandler() {
  if (typeof document.on === "function") {
    document.on("click", "[data-action]", function (event, target) {
      handleClick(target || this || (event && event.target));
    });
    return true;
  }

  if (typeof document.addEventListener === "function") {
    document.addEventListener("click", function (event) {
      handleClick(event.target);
    });
    return true;
  }

  return false;
}

function bindDirectOpenHandler() {
  if (!document || typeof document.querySelector !== "function") {
    return false;
  }

  const openButton = document.querySelector('[data-action="open-file"]');
  if (!openButton || typeof openButton.addEventListener !== "function") {
    return false;
  }

  openButton.addEventListener("click", function (event) {
    handleClick((event && event.currentTarget) || this);
  });
  return true;
}

function bindSearchHandlers() {
  const searchInputEl = searchInput();
  if (!searchInputEl) {
    return false;
  }

  const searchPrevBtn = document.querySelector("[data-search-prev]");
  const searchNextBtn = document.querySelector("[data-search-next]");
  const searchCloseBtn = document.querySelector("[data-search-close]");
  const searchCaseCheckbox = document.querySelector("[data-search-case-sensitive]");

  if (typeof searchInputEl.addEventListener === "function") {
    searchInputEl.addEventListener("input", handleSearchInput);
  }

  if (searchCaseCheckbox && typeof searchCaseCheckbox.addEventListener === "function") {
    if (searchCaseSensitive) {
      searchCaseCheckbox.classList.add("active");
      searchCaseCheckbox.setAttribute("aria-pressed", "true");
    } else {
      searchCaseCheckbox.classList.remove("active");
      searchCaseCheckbox.setAttribute("aria-pressed", "false");
    }
    searchCaseCheckbox.addEventListener("click", function () {
      searchCaseSensitive = !searchCaseSensitive;
      if (searchCaseSensitive) {
        searchCaseCheckbox.classList.add("active");
        searchCaseCheckbox.setAttribute("aria-pressed", "true");
      } else {
        searchCaseCheckbox.classList.remove("active");
        searchCaseCheckbox.setAttribute("aria-pressed", "false");
      }
      performSearch();
    });
  }

  if (searchPrevBtn && typeof searchPrevBtn.addEventListener === "function") {
    searchPrevBtn.addEventListener("click", function (_event) {
      navigateToPrevMatch();
    });
  }

  if (searchNextBtn && typeof searchNextBtn.addEventListener === "function") {
    searchNextBtn.addEventListener("click", function (_event) {
      navigateToNextMatch();
    });
  }

  if (searchCloseBtn && typeof searchCloseBtn.addEventListener === "function") {
    searchCloseBtn.addEventListener("click", function (_event) {
      hideSearchPanel();
    });
  }

  return true;
}

function bindMarkdownContextMenuHandler() {
  if (markdownContextMenuBound) {
    return true;
  }

  const host = markdownBodyHost();
  const selectionHost = document && typeof document.querySelector === "function"
    ? document.querySelector("[data-markdown-selection-host]")
    : null;
  const body = markdownBody();
  const target = selectionHost || body || host;

  if (!target || typeof target.addEventListener !== "function") {
    return false;
  }

  target.addEventListener("contextmenu", function (event) {
    return handleMarkdownContextMenu(event, (event && event.target) || target);
  });
  markdownContextMenuBound = true;
  return true;
}

function handleSearchInput(event) {
  if (!event) {
    return;
  }

  if (event.type === "input") {
    performSearch();
  }
}

function bindKeyboardShortcuts() {
  let bound = false;
  const runningInNode = typeof process !== "undefined" && process && process.versions && process.versions.node;
  if (!runningInNode && document && typeof document.on === "function") {
    try {
      document.on("^keydown", handleKeyboardShortcuts);
      return true;
    } catch (_error) {}
  }

  if (!runningInNode && typeof Window !== "undefined" && Window.this && typeof Window.this.on === "function") {
    try {
      Window.this.on("keydown", handleKeyboardShortcuts);
      bound = true;
    } catch (_error) {}
    if (document && typeof document.addEventListener === "function") {
      document.addEventListener("keydown", handleKeyboardShortcuts);
      bound = true;
    }
  } else {
    if (document && typeof document.addEventListener === "function") {
      document.addEventListener("keydown", handleKeyboardShortcuts);
      bound = true;
    }
  }

  return bound;
}

function isSearchShortcut(event) {
  if (!event) {
    return false;
  }

  const key = typeof event.key === "string" ? event.key.toLowerCase() : "";
  const code = typeof event.code === "string" ? event.code.toLowerCase() : "";
  const keyCode = typeof event.keyCode === "number" ? event.keyCode : -1;
  const hasModifier = event.ctrlKey || event.metaKey;
  return hasModifier && (key === "f" || code === "keyf" || keyCode === 70);
}

function isOpenFileShortcut(event) {
  if (!event) {
    return false;
  }

  const key = typeof event.key === "string" ? event.key.toLowerCase() : "";
  const code = typeof event.code === "string" ? event.code.toLowerCase() : "";
  const keyCode = typeof event.keyCode === "number" ? event.keyCode : -1;
  const hasModifier = event.ctrlKey || event.metaKey;
  return hasModifier && (key === "o" || code === "keyo" || keyCode === 79);
}

function isEnterKey(event) {
  if (!event) {
    return false;
  }

  const key = typeof event.key === "string" ? event.key.toLowerCase() : "";
  return key === "enter";
}

function isEscapeKey(event) {
  if (!event) {
    return false;
  }

  const key = typeof event.key === "string" ? event.key.toLowerCase() : "";
  const code = typeof event.code === "string" ? event.code.toLowerCase() : "";
  const keyCode = typeof event.keyCode === "number" ? event.keyCode : -1;
  return key === "escape" || key === "esc" || code === "escape" || keyCode === 27;
}

function isOverlayVisible(overlay) {
  if (!overlay) {
    return false;
  }

  if (overlay.hidden) {
    return false;
  }

  const hiddenAttr = typeof overlay.getAttribute === "function"
    ? overlay.getAttribute("hidden")
    : null;
  if (hiddenAttr !== null) {
    return false;
  }

  if (overlay.style && overlay.style.display === "none") {
    return false;
  }

  return true;
}

function handleKeyboardShortcuts(event) {
  if (!event) {
    return;
  }

  const searchInputEl = searchInput();
  const isSearchFocused = searchInputEl && document.activeElement === searchInputEl;

  if (isSearchShortcut(event)) {
    if (typeof event.preventDefault === "function") {
      event.preventDefault();
    }
    toggleSearchPanel();
    return;
  }

  if (isOpenFileShortcut(event)) {
    if (typeof event.preventDefault === "function") {
      event.preventDefault();
    }

    const button = openFileButton();
    if (button && typeof button.click === "function") {
      button.click();
      return;
    }

    requestOpenFile();
    return;
  }

  if (isEscapeKey(event)) {
    const overlay = aboutOverlayElement();
    if (isOverlayVisible(overlay)) {
      event.preventDefault();
      setAboutOverlayOpen(false);
      return;
    }

    const errorOverlay = errorOverlayElement();
    if (isOverlayVisible(errorOverlay)) {
      event.preventDefault();
      setErrorOverlayOpen(false);
      requestErrorDismiss();
      return;
    }
  }

  if (searchVisible) {
    if (isEscapeKey(event)) {
      event.preventDefault();
      hideSearchPanel();
      return;
    }

    if (isSearchFocused && isEnterKey(event)) {
      if (event.shiftKey) {
        event.preventDefault();
        navigateToPrevMatch();
      } else {
        event.preventDefault();
        navigateToNextMatch();
      }
      return;
    }
  }

  handleCopyShortcut(event);
}

function scheduleSciterRepaint() {
  var doRepaint = function () {
    if (typeof Window !== "undefined" && Window.this && typeof Window.this.update === "function") {
      try {
        Window.this.update();
      } catch (_error) {
        // Ignore refresh failures and keep the viewer usable.
      }
    }
  };

  // Defer one event-loop tick so Sciter's layout pass finishes before the repaint is
  // requested. Calling Window.this.update() synchronously during document load can
  // race against layout completion and leave the viewport blank on loaded documents.
  if (typeof setTimeout === "function") {
    setTimeout(doRepaint, 0);
  } else {
    doRepaint();
  }
}

var _pendingDropFiles = [];

function dispatchPendingDropFiles() {
  if (_pendingDropFiles.length === 0) {
    return;
  }
  if (typeof Window !== "undefined" && Window.this && typeof Window.this.xcall === "function") {
    Window.this.xcall.apply(null, ["open-dropped-files"].concat(_pendingDropFiles));
  }
  _pendingDropFiles = [];
}

var FileDropTarget = null;
if (typeof Element !== "undefined") {
  FileDropTarget = class extends Element {
    _dndFiles = [];

    componentDidMount() {
      this.on("dragaccept", function(evt) {
        if (evt.detail && evt.detail.dataType === "file") {
          this._dndFiles = Array.isArray(evt.detail.data)
            ? evt.detail.data.filter(Boolean)
            : evt.detail.data ? [evt.detail.data] : [];
          evt.stopPropagation();
          return true;
        }
        return false;
      }.bind(this));

      this.on("dragenter", function(evt) {
        evt.stopPropagation();
      });

      this.on("dragleave", function(evt) {
        evt.stopPropagation();
      });

      this.on("drag", function(evt) {
        evt.stopPropagation();
      });

      this.on("drop", function(evt) {
        evt.stopPropagation();
        if (this._dndFiles.length > 0) {
          _pendingDropFiles = this._dndFiles.slice();
          if (typeof setTimeout === "function") {
            setTimeout(dispatchPendingDropFiles, 0);
          } else {
            dispatchPendingDropFiles();
          }
        }
        this._dndFiles = [];
      }.bind(this));
    }
  };
}

function currentFileElement() {
  if (!document || typeof document.querySelector !== "function") {
    return null;
  }

  return document.querySelector("[data-current-file]");
}

function handleRecentFileMenuClick(_event, item) {
  const actionTarget = normalizeEventTarget(item);
  if (shouldIgnoreTarget(actionTarget)) {
    return;
  }

  const index = actionTarget && typeof actionTarget.getAttribute === "function"
    ? actionTarget.getAttribute("data-recent-index")
    : null;
  if (index === null || index === "") {
    return;
  }

  requestOpenRecentFile(index);
}

function showRecentFilesPopup(target) {
  var menu = recentFilesMenu();
  if (!menu || !menu.childElementCount) return;
  var anchor = target || currentFileElement();
  if (!anchor || typeof anchor.popup !== "function") return;
  anchor.popup(menu, { anchorAt: 1, popupAt: 7 });
}

function showRecentFilesFromNativeCaption() {
  // Invoked from Win32 WM_NCRBUTTONUP/HTCAPTION bridge when Sciter's
  // window-caption role absorbs contextmenu events in the title bar.
  showRecentFilesPopup(currentFileElement());
}

function bindTitlebarRecentFilesContextMenu() {
  var fileNameEl = currentFileElement();
  if (!fileNameEl || typeof fileNameEl.addEventListener !== "function") return false;

  // Keep a DOM fallback for cases where contextmenu is delivered to the
  // file-name element (non-caption contexts or future title bar changes).
  fileNameEl.addEventListener("contextmenu", function(evt) {
    evt.preventDefault();
    showRecentFilesPopup(fileNameEl);
    return true;
  });
  return true;
}

function bindRecentFilesMenuHandler() {
  if (!document || typeof document.on !== "function") {
    return false;
  }

  document.on("click", "menu#recent-files-menu > li", function(event, item) {
    handleRecentFileMenuClick(event, this || item);
  });
  return true;
}

function bindErrorOverlayHandler() {
  const button = errorOkButton();
  if (!button || typeof button.addEventListener !== "function") {
    return false;
  }

  button.addEventListener("click", function () {
    setErrorOverlayOpen(false);
    requestErrorDismiss();
  });
  return true;
}

function initializeInteractions() {
  if (!interactionsInitialized) {
    interactionsInitialized = true;
    bindDelegatedClickHandler();
    bindDirectOpenHandler();
    bindSearchHandlers();
    bindKeyboardShortcuts();
    bindTitlebarRecentFilesContextMenu();
    bindRecentFilesMenuHandler();
  }

  bindMarkdownContextMenuHandler();
  bindErrorOverlayHandler();
}

function registerInitialRepaint() {
  if (repaintRegistrationInitialized) {
    return;
  }

  repaintRegistrationInitialized = true;

  // Sciter runs document scripts after parsing, so event bindings can be installed
  // immediately. The repaint request needs to wait for the posted "ready" phase so the
  // window is constructed and visible before Window.this.update() runs.
  if (document && typeof document.on === "function") {
    document.on("ready", scheduleSciterRepaint);
    return;
  }

  if (document && typeof document.addEventListener === "function") {
    document.addEventListener("DOMContentLoaded", scheduleSciterRepaint);
    return;
  }

  scheduleSciterRepaint();
}

if (typeof globalThis !== "undefined") {
  globalThis.__mdlumaTestHooks = Object.assign({}, globalThis.__mdlumaTestHooks, {
    handleClick,
    handleMarkdownContextMenu,
    hasLoadedDocument,
    markdownContextMenuHtml,
    handleRecentFileMenuClick,
    setAboutOverlayOpen,
    setErrorOverlayOpen,
    showRecentFilesFromNativeCaption,
    showRecentFilesPopup,
    requestErrorDismiss,
    requestExternalEditorSetting,
  });

  // Stable global entrypoint for native caption right-click handling.
  globalThis.__mdlumaShowRecentFilesFromNativeCaption = showRecentFilesFromNativeCaption;
}

if (document && typeof document.on === "function") {
  document.on("ready", handleDocumentReady);
}

handleDocumentReady();

registerInitialRepaint();
