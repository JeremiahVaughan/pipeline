(function () {
  function sendMessage(message) {
    const ws = window.WS;
    if (ws && typeof ws.send === "function") {
      ws.send(message);
      return true;
    }
    console.warn("[custom-htmx] WS.send unavailable");
    return false;
  }

  function collectFormValues(form) {
    if (!form) return [];
    const fields = Array.from(form.querySelectorAll("input, select, textarea"));
    const values = [];
    fields.forEach((field) => {
      if (field.disabled) return;
      if (field.tagName === "INPUT") {
        const type = (field.getAttribute("type") || "").toLowerCase();
        if (["submit", "button", "reset", "image"].includes(type)) {
          return;
        }
        if (["checkbox", "radio"].includes(type)) {
          values.push(field.checked ? field.value : "");
          return;
        }
      }
      values.push(field.value ?? "");
    });
    return values;
  }

  function buildPatchMessage(el) {
    const patch =
      el.getAttribute("hx-patch") || el.getAttribute("data-hx-patch");
    if (!patch) return null;
    const form = el.closest("form");
    const values = collectFormValues(form);
    return [patch, ...values].join(":");
  }

  function defaultTriggerFor(el) {
    const tag = el.tagName;
    if (tag === "FORM") return "submit";
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") {
      return "change";
    }
    return "click";
  }

  function parseTriggers(el) {
    const trigger =
      el.getAttribute("hx-trigger") || el.getAttribute("data-hx-trigger");
    if (!trigger) {
      return [defaultTriggerFor(el)];
    }
    return trigger
      .split(",")
      .map((part) => part.trim())
      .filter(Boolean)
      .map((part) => part.split(/\s+/)[0]);
  }

  function bindElement(el) {
    if (el.dataset.customHtmxBound === "true") return;
    el.dataset.customHtmxBound = "true";
    const triggers = parseTriggers(el);
    triggers.forEach((eventName) => {
      el.addEventListener(eventName, (event) => {
        if (eventName === "submit") {
          event.preventDefault();
        }
        const message = buildPatchMessage(el);
        if (!message) return;
        sendMessage(message);
      });
    });
  }

  function scan(root) {
    if (!root || root.nodeType !== Node.ELEMENT_NODE) return;
    if (root.matches("[hx-patch], [data-hx-patch]")) {
      bindElement(root);
    }
    root
      .querySelectorAll("[hx-patch], [data-hx-patch]")
      .forEach(bindElement);
  }

  function parseFragment(html) {
    const template = document.createElement("template");
    template.innerHTML = html;
    return template.content;
  }

  function resolveTarget(node, selector) {
    if (selector) {
      return document.querySelector(selector);
    }
    if (node.id) {
      return document.getElementById(node.id);
    }
    return null;
  }

  function swapNode(target, node, swap) {
    switch (swap) {
      case "true":
      case "outerHTML":
        target.replaceWith(node);
        return;
      case "innerHTML":
        target.innerHTML = "";
        target.append(...node.childNodes);
        return;
      case "beforebegin":
        target.before(node);
        return;
      case "afterbegin":
        target.prepend(node);
        return;
      case "beforeend":
        target.append(node);
        return;
      case "afterend":
        target.after(node);
        return;
      default:
        target.replaceWith(node);
    }
  }

  function applyOobSwaps(fragment) {
    if (!fragment) return 0;
    const nodes = fragment.querySelectorAll("[hx-swap-oob], [data-hx-swap-oob]");
    let applied = 0;

    nodes.forEach((node) => {
      const attr = node.getAttribute("hx-swap-oob") || node.getAttribute("data-hx-swap-oob") || "true";
      const spec = attr.trim() || "true";
      const parts = spec.split(":");
      const swap = parts[0].trim() || "true";
      const selector = parts.length > 1 ? parts.slice(1).join(":").trim() : "";
      const target = resolveTarget(node, selector || null);
      if (!target) {
        return;
      }
      const clone = node.cloneNode(true);
      swapNode(target, clone, swap);
      applied += 1;
    });

    return applied;
  }
  window.customHtmx = {
    parseFragment,
    applyOobSwaps,
    scan,
  };

  function startObserving() {
    if (!document.body) return;
    scan(document.body);
    const observer = new MutationObserver((mutations) => {
      mutations.forEach((mutation) => {
        mutation.addedNodes.forEach((node) => scan(node));
      });
    });
    observer.observe(document.body, { childList: true, subtree: true });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", startObserving);
  } else {
    startObserving();
  }

  document.dispatchEvent(new CustomEvent("customhtmx:ready"));
})();
