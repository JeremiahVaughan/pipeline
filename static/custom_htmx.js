(function () {
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
console.log("todo remove ViFuR");
  window.customHtmx = {
    parseFragment,
    applyOobSwaps,
  };
  document.dispatchEvent(new CustomEvent("customhtmx:ready"));
})();
