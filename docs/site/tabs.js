// Turn a block of per-board sections into tabs, on the site only.
//
// A page writes plain markdown wrapped in a div:
//
//     <div class="tabs" data-group="board">
//
//     ### Arduino Uno
//
//     ...
//
//     ### ESP32
//
//     ...
//
//     </div>
//
// GitHub drops the div and shows every section one after the other under its
// heading, which reads fine. Here, the headings at the level of the block's
// first heading become the tab labels, and everything up to the next such
// heading is that tab's panel. Deeper headings stay inside their panel.
//
// Blocks sharing a `data-group` switch together, so picking "Arduino Uno" once
// flips every board-specific section on the page, and the choice is remembered
// across pages. A link to an anchor inside a hidden panel opens that panel.
(function () {
  "use strict";

  const STORAGE_PREFIX = "modit-tab-";
  const HEADING = /^H[1-6]$/;

  function remembered(group) {
    try {
      return localStorage.getItem(STORAGE_PREFIX + group);
    } catch (_) {
      return null;
    }
  }

  function remember(group, label) {
    try {
      localStorage.setItem(STORAGE_PREFIX + group, label);
    } catch (_) {
      // Private windows and blocked storage: the tabs still work, unremembered.
    }
  }

  let blockCount = 0;

  function build(block) {
    const first = Array.from(block.children).find((el) => HEADING.test(el.tagName));
    if (!first) return null;
    const level = first.tagName;
    const blockId = "tabs-" + blockCount++;

    const tabs = [];
    let current = null;
    for (const el of Array.from(block.children)) {
      if (el.tagName === level) {
        const panel = document.createElement("div");
        panel.className = "tab-panel";
        panel.setAttribute("role", "tabpanel");
        current = { label: el.textContent.trim(), heading: el, panel };
        tabs.push(current);
      } else if (current) {
        current.panel.appendChild(el);
      }
    }
    if (tabs.length < 2) return null;

    const list = document.createElement("div");
    list.className = "tab-list";
    list.setAttribute("role", "tablist");

    tabs.forEach((tab, i) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "tab";
      button.textContent = tab.label;
      button.id = `${blockId}-tab-${i}`;
      button.setAttribute("role", "tab");
      button.setAttribute("aria-controls", `${blockId}-panel-${i}`);
      tab.panel.id = `${blockId}-panel-${i}`;
      tab.panel.setAttribute("aria-labelledby", button.id);
      tab.button = button;
      list.appendChild(button);

      // The heading's anchor id stays reachable: it moves onto the panel, so
      // `#arduino-uno` still lands on the right content.
      if (tab.heading.id) {
        tab.panel.dataset.anchor = tab.heading.id;
      }
      tab.heading.remove();
    });

    block.appendChild(list);
    tabs.forEach((tab) => block.appendChild(tab.panel));

    // Keep headings inside a panel out of mdBook's "on this page" sidebar: every
    // board repeats them ("Power", "Power"), and a sidebar entry for a hidden
    // panel scrolls nowhere. mdBook lists a heading only if the heading itself
    // carries the id, so the id moves onto its anchor link, where `#power` still
    // resolves. If mdBook changes that rule, the cost is duplicate entries.
    for (const tab of tabs) {
      for (const heading of tab.panel.querySelectorAll("h2, h3, h4, h5, h6")) {
        const link = heading.querySelector("a.header");
        if (heading.id && link && !link.id) {
          link.id = heading.id;
          heading.removeAttribute("id");
        }
      }
    }
    block.classList.add("tabs-ready");

    return { group: block.dataset.group || null, tabs, list };
  }

  function select(block, label, focus) {
    const index = block.tabs.findIndex((t) => t.label === label);
    if (index < 0) return false;
    block.tabs.forEach((tab, i) => {
      const on = i === index;
      tab.button.setAttribute("aria-selected", on ? "true" : "false");
      tab.button.tabIndex = on ? 0 : -1;
      tab.panel.hidden = !on;
    });
    if (focus) block.tabs[index].button.focus();
    return true;
  }

  function init() {
    const blocks = Array.from(document.querySelectorAll("div.tabs"))
      .map(build)
      .filter(Boolean);
    if (!blocks.length) return;

    function selectGroup(origin, label, focus) {
      for (const block of blocks) {
        if (block === origin) {
          select(block, label, focus);
        } else if (origin.group && block.group === origin.group) {
          select(block, label, false);
        }
      }
      if (origin.group) remember(origin.group, label);
    }

    for (const block of blocks) {
      const saved = block.group && remembered(block.group);
      if (!(saved && select(block, saved, false))) {
        select(block, block.tabs[0].label, false);
      }

      block.tabs.forEach((tab, i) => {
        tab.button.addEventListener("click", () => selectGroup(block, tab.label, false));
        tab.button.addEventListener("keydown", (event) => {
          const n = block.tabs.length;
          const step = { ArrowRight: 1, ArrowLeft: n - 1 }[event.key];
          let target = null;
          if (step) target = (i + step) % n;
          if (event.key === "Home") target = 0;
          if (event.key === "End") target = n - 1;
          if (target === null) return;
          event.preventDefault();
          selectGroup(block, block.tabs[target].label, true);
        });
      });
    }

    function revealHash() {
      const id = decodeURIComponent(location.hash.slice(1));
      if (!id) return;
      for (const block of blocks) {
        for (const tab of block.tabs) {
          const hit =
            tab.panel.dataset.anchor === id || tab.panel.querySelector(`[id="${CSS.escape(id)}"]`);
          if (hit) {
            selectGroup(block, tab.label, false);
            const target = document.getElementById(id) || tab.panel;
            target.scrollIntoView();
            return;
          }
        }
      }
    }
    window.addEventListener("hashchange", revealHash);
    revealHash();
  }

  // mdBook loads additional scripts at the end of <body>, so the content is
  // already parsed. Running now rather than on DOMContentLoaded matters: mdBook's
  // own sidebar collects the page's headings on DOMContentLoaded, and has to see
  // the page with the tab labels already gone.
  if (document.querySelector("main")) {
    init();
  } else {
    document.addEventListener("DOMContentLoaded", init);
  }
})();
